//! Shared, bounded, progressive materialization of missing catalog tracks.

use crate::catalog_store::{CatalogStore, TrackAvailability};
use crate::config::ProxyModeSettings;
use crate::downloader::{DownloadPriority, Downloader};
use anyhow::{Context, Result};
use bytes::Bytes;
use futures::{Stream, StreamExt};
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex, Weak};
use std::task::{Context as TaskContext, Poll};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::AsyncWriteExt;
use tokio::sync::{Notify, Semaphore};
use tracing::{info, warn};

const RESPONSE_CHUNK_SIZE: usize = 64 * 1024;
const RECENT_JOB_LIMIT: usize = 50;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProxyJobPhase {
    Queued,
    WaitingForCapacity,
    Connecting,
    Downloading,
    Validating,
    Saving,
    WaitingForPublication,
    Completed,
    Failed,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProxyJobStatus {
    pub track_id: String,
    pub track_name: Option<String>,
    pub album_id: Option<String>,
    pub album_name: Option<String>,
    pub priority: &'static str,
    pub phase: ProxyJobPhase,
    pub bytes_downloaded: u64,
    pub bytes_streamed: u64,
    pub total_bytes: Option<u64>,
    pub active_streams: usize,
    pub started_at_ms: u64,
    pub updated_at_ms: u64,
    pub finished_at_ms: Option<u64>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProxyMaterializerStatus {
    pub enabled: bool,
    pub active: Vec<ProxyJobStatus>,
    pub recent: Vec<ProxyJobStatus>,
    pub memory_used_bytes: u64,
    pub memory_limit_bytes: u64,
    pub foreground_active: usize,
    pub foreground_limit: usize,
    pub prefetch_active: usize,
    pub prefetch_limit: usize,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn priority_name(priority: DownloadPriority) -> &'static str {
    match priority {
        DownloadPriority::Foreground => "foreground",
        DownloadPriority::Normal => "normal",
        DownloadPriority::Prefetch => "prefetch",
    }
}

#[derive(Clone, Debug)]
pub struct TrackStreamMetadata {
    pub content_length: u64,
    pub content_type: String,
}

#[derive(Default)]
struct BufferState {
    metadata: Option<TrackStreamMetadata>,
    extension: Option<String>,
    bytes: Vec<u8>,
    complete: bool,
    error: Option<String>,
}

#[derive(Default)]
struct ConsumptionState {
    ranges: Vec<(u64, u64)>,
    unique_bytes: u64,
    active_streams: usize,
}

pub struct InFlightTrack {
    track_id: String,
    state: Mutex<BufferState>,
    changed: Notify,
    _reservation: Mutex<Option<MemoryReservation>>,
    status: Mutex<ProxyJobStatus>,
    consumption: Mutex<ConsumptionState>,
}

impl InFlightTrack {
    fn new(track_id: String, priority: DownloadPriority) -> Self {
        let started_at_ms = now_ms();
        Self {
            track_id: track_id.clone(),
            state: Mutex::new(BufferState::default()),
            changed: Notify::new(),
            _reservation: Mutex::new(None),
            consumption: Mutex::new(ConsumptionState::default()),
            status: Mutex::new(ProxyJobStatus {
                track_id,
                track_name: None,
                album_id: None,
                album_name: None,
                priority: priority_name(priority),
                phase: ProxyJobPhase::Queued,
                bytes_downloaded: 0,
                bytes_streamed: 0,
                total_bytes: None,
                active_streams: 0,
                started_at_ms,
                updated_at_ms: started_at_ms,
                finished_at_ms: None,
                error: None,
            }),
        }
    }

    fn set_phase(&self, phase: ProxyJobPhase) {
        let mut status = self.status.lock().expect("track status mutex poisoned");
        status.phase = phase;
        status.updated_at_ms = now_ms();
    }

    fn set_metadata_status(&self, name: String, album_id: String, album_name: String) {
        let mut status = self.status.lock().expect("track status mutex poisoned");
        status.track_name = Some(name);
        status.album_id = Some(album_id);
        status.album_name = Some(album_name);
        status.updated_at_ms = now_ms();
    }

    fn set_total_bytes(&self, total: u64) {
        let mut status = self.status.lock().expect("track status mutex poisoned");
        status.total_bytes = Some(total);
        status.updated_at_ms = now_ms();
    }

    fn add_downloaded_bytes(&self, bytes: usize) {
        let mut status = self.status.lock().expect("track status mutex poisoned");
        status.bytes_downloaded = status.bytes_downloaded.saturating_add(bytes as u64);
        status.updated_at_ms = now_ms();
    }

    fn start_stream(&self) -> bool {
        let active_streams = {
            let mut consumption = self
                .consumption
                .lock()
                .expect("track consumption mutex poisoned");
            consumption.active_streams += 1;
            consumption.active_streams
        };
        let mut status = self.status.lock().expect("track status mutex poisoned");
        status.active_streams = active_streams;
        status.updated_at_ms = now_ms();
        drop(status);
        true
    }

    fn finish_stream(&self) {
        let active_streams = {
            let mut consumption = self
                .consumption
                .lock()
                .expect("track consumption mutex poisoned");
            consumption.active_streams = consumption.active_streams.saturating_sub(1);
            consumption.active_streams
        };
        let mut status = self.status.lock().expect("track status mutex poisoned");
        status.active_streams = active_streams;
        status.updated_at_ms = now_ms();
        drop(status);
    }

    fn record_streamed_range(&self, start: u64, end: u64) {
        if start >= end {
            return;
        }
        let unique_bytes = {
            let mut consumption = self
                .consumption
                .lock()
                .expect("track consumption mutex poisoned");
            consumption.ranges.push((start, end));
            consumption.ranges.sort_unstable_by_key(|range| range.0);
            let mut merged: Vec<(u64, u64)> = Vec::with_capacity(consumption.ranges.len());
            for (range_start, range_end) in consumption.ranges.drain(..) {
                if let Some(last) = merged.last_mut() {
                    if range_start <= last.1 {
                        last.1 = last.1.max(range_end);
                        continue;
                    }
                }
                merged.push((range_start, range_end));
            }
            consumption.unique_bytes = merged
                .iter()
                .map(|(range_start, range_end)| range_end.saturating_sub(*range_start))
                .sum();
            consumption.ranges = merged;
            consumption.unique_bytes
        };
        let mut status = self.status.lock().expect("track status mutex poisoned");
        status.bytes_streamed = unique_bytes;
        status.updated_at_ms = now_ms();
        drop(status);
    }

    fn finish(&self, phase: ProxyJobPhase, error: Option<String>) {
        let finished_at_ms = now_ms();
        let mut status = self.status.lock().expect("track status mutex poisoned");
        status.phase = phase;
        status.updated_at_ms = finished_at_ms;
        status.finished_at_ms = Some(finished_at_ms);
        status.error = error;
    }

    fn status(&self) -> ProxyJobStatus {
        self.status
            .lock()
            .expect("track status mutex poisoned")
            .clone()
    }

    pub async fn metadata(&self) -> Result<TrackStreamMetadata> {
        loop {
            let notified = self.changed.notified();
            {
                let state = self.state.lock().expect("track buffer mutex poisoned");
                if let Some(error) = &state.error {
                    anyhow::bail!(error.clone());
                }
                if let Some(metadata) = &state.metadata {
                    return Ok(metadata.clone());
                }
            }
            notified.await;
        }
    }

    pub fn range_stream(
        self: Arc<Self>,
        start: u64,
        length: u64,
    ) -> Pin<Box<dyn Stream<Item = io::Result<Bytes>> + Send>> {
        let end = start.saturating_add(length);
        let registered = self.start_stream();
        if !registered {
            return Box::pin(futures::stream::once(async {
                Err(io::Error::other("proxy track was discarded"))
            }));
        }
        let tracked = self.clone();
        let inner = Box::pin(futures::stream::unfold(
            (self, start, end),
            |(track, cursor, end)| async move {
                if cursor >= end {
                    return None;
                }
                loop {
                    let notified = track.changed.notified();
                    let outcome = {
                        let state = track.state.lock().expect("track buffer mutex poisoned");
                        if let Some(error) = &state.error {
                            Some(Err(io::Error::other(error.clone())))
                        } else if cursor < state.bytes.len() as u64 {
                            let available_end = (state.bytes.len() as u64)
                                .min(end)
                                .min(cursor.saturating_add(RESPONSE_CHUNK_SIZE as u64));
                            Some(Ok(Bytes::copy_from_slice(
                                &state.bytes[cursor as usize..available_end as usize],
                            )))
                        } else if state.complete {
                            Some(Err(io::Error::new(
                                io::ErrorKind::UnexpectedEof,
                                "proxy track completed before requested range",
                            )))
                        } else {
                            None
                        }
                    };
                    match outcome {
                        Some(Ok(bytes)) => {
                            let next = cursor + bytes.len() as u64;
                            track.record_streamed_range(cursor, next);
                            return Some((Ok(bytes), (track.clone(), next, end)));
                        }
                        Some(Err(error)) => return Some((Err(error), (track.clone(), end, end))),
                        None => notified.await,
                    }
                }
            },
        ));
        Box::pin(TrackedRangeStream {
            inner,
            track: tracked,
        })
    }
}

struct TrackedRangeStream {
    inner: Pin<Box<dyn Stream<Item = io::Result<Bytes>> + Send>>,
    track: Arc<InFlightTrack>,
}

impl Stream for TrackedRangeStream {
    type Item = io::Result<Bytes>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Option<Self::Item>> {
        self.inner.as_mut().poll_next(cx)
    }
}

impl Drop for TrackedRangeStream {
    fn drop(&mut self) {
        self.track.finish_stream();
    }
}

struct MemoryBudget {
    limit: u64,
    foreground_reserve: u64,
    used: Mutex<u64>,
    changed: Notify,
}

impl MemoryBudget {
    async fn reserve(
        self: &Arc<Self>,
        bytes: u64,
        priority: DownloadPriority,
        timeout: Duration,
    ) -> Result<MemoryReservation> {
        let reserve = async {
            loop {
                let notified = self.changed.notified();
                {
                    let mut used = self.used.lock().expect("memory budget mutex poisoned");
                    let limit = if priority == DownloadPriority::Prefetch {
                        self.limit.saturating_sub(self.foreground_reserve)
                    } else {
                        self.limit
                    };
                    if used.saturating_add(bytes) <= limit {
                        *used += bytes;
                        return MemoryReservation {
                            bytes,
                            budget: Arc::downgrade(self),
                        };
                    }
                }
                notified.await;
            }
        };
        tokio::time::timeout(timeout, reserve)
            .await
            .context("proxy memory capacity timed out")
    }
}

struct MemoryReservation {
    bytes: u64,
    budget: Weak<MemoryBudget>,
}

impl Drop for MemoryReservation {
    fn drop(&mut self) {
        if let Some(budget) = self.budget.upgrade() {
            let mut used = budget.used.lock().expect("memory budget mutex poisoned");
            *used = used.saturating_sub(self.bytes);
            drop(used);
            budget.changed.notify_waiters();
        }
    }
}

pub struct TrackMaterializer {
    downloader: Arc<dyn Downloader>,
    catalog: Arc<dyn CatalogStore>,
    settings: ProxyModeSettings,
    media: Weak<super::MediaManager>,
    jobs: Mutex<HashMap<String, Arc<InFlightTrack>>>,
    recent_jobs: Mutex<VecDeque<ProxyJobStatus>>,
    budget: Arc<MemoryBudget>,
    foreground_slots: Semaphore,
    prefetch_slots: Semaphore,
}

impl TrackMaterializer {
    pub fn new(
        downloader: Arc<dyn Downloader>,
        catalog: Arc<dyn CatalogStore>,
        settings: ProxyModeSettings,
        media: Weak<super::MediaManager>,
    ) -> Arc<Self> {
        Arc::new(Self {
            downloader,
            catalog,
            budget: Arc::new(MemoryBudget {
                limit: settings.memory_budget_bytes,
                foreground_reserve: settings.max_track_size_bytes,
                used: Mutex::new(0),
                changed: Notify::new(),
            }),
            foreground_slots: Semaphore::new(settings.max_foreground_downloads),
            prefetch_slots: Semaphore::new(settings.max_prefetch_downloads),
            settings,
            media,
            jobs: Mutex::new(HashMap::new()),
            recent_jobs: Mutex::new(VecDeque::new()),
        })
    }

    pub fn status(&self) -> ProxyMaterializerStatus {
        let mut active = self
            .jobs
            .lock()
            .expect("materializer jobs mutex poisoned")
            .values()
            .map(|job| job.status())
            .collect::<Vec<_>>();
        active.sort_by_key(|job| job.started_at_ms);
        let recent = self
            .recent_jobs
            .lock()
            .expect("materializer history mutex poisoned")
            .iter()
            .rev()
            .cloned()
            .collect();
        let memory_used_bytes = *self
            .budget
            .used
            .lock()
            .expect("memory budget mutex poisoned");
        let foreground_limit = self.settings.max_foreground_downloads;
        let prefetch_limit = self.settings.max_prefetch_downloads;
        ProxyMaterializerStatus {
            enabled: true,
            foreground_active: foreground_limit
                .saturating_sub(self.foreground_slots.available_permits()),
            foreground_limit,
            prefetch_active: prefetch_limit.saturating_sub(self.prefetch_slots.available_permits()),
            prefetch_limit,
            active,
            recent,
            memory_used_bytes,
            memory_limit_bytes: self.budget.limit,
        }
    }

    pub fn get_or_start(
        self: &Arc<Self>,
        track_id: &str,
        priority: DownloadPriority,
    ) -> Arc<InFlightTrack> {
        let mut jobs = self.jobs.lock().expect("materializer jobs mutex poisoned");
        if let Some(existing) = jobs.get(track_id) {
            return existing.clone();
        }
        let track = Arc::new(InFlightTrack::new(track_id.to_string(), priority));
        jobs.insert(track_id.to_string(), track.clone());
        drop(jobs);

        let materializer = self.clone();
        let task_track = track.clone();
        tokio::spawn(async move {
            if let Err(error) = materializer
                .download_and_publish(task_track.clone(), priority)
                .await
            {
                warn!(track_id = %task_track.track_id, %error, "Proxy materialization failed");
                task_track.finish(ProxyJobPhase::Failed, Some(error.to_string()));
                let mut state = task_track
                    .state
                    .lock()
                    .expect("track buffer mutex poisoned");
                state.error = Some(error.to_string());
                drop(state);
                task_track.changed.notify_waiters();
            }
            let completed_status = task_track.status();
            let mut recent = materializer
                .recent_jobs
                .lock()
                .expect("materializer history mutex poisoned");
            recent.push_back(completed_status);
            while recent.len() > RECENT_JOB_LIMIT {
                recent.pop_front();
            }
            drop(recent);
            materializer
                .jobs
                .lock()
                .expect("materializer jobs mutex poisoned")
                .remove(&task_track.track_id);
        });
        track
    }

    async fn download_and_publish(
        self: &Arc<Self>,
        track: Arc<InFlightTrack>,
        priority: DownloadPriority,
    ) -> Result<()> {
        let timeout = Duration::from_secs(self.settings.no_progress_timeout_secs);
        track.set_phase(ProxyJobPhase::WaitingForCapacity);
        let _slot = match priority {
            DownloadPriority::Prefetch => self.prefetch_slots.acquire().await?,
            _ => self.foreground_slots.acquire().await?,
        };
        if let Some(resolved) = self.catalog.get_resolved_track(&track.track_id)? {
            track.set_metadata_status(resolved.track.name, resolved.album.id, resolved.album.name);
        }
        track.set_phase(ProxyJobPhase::Connecting);
        let mut download = tokio::time::timeout(
            timeout,
            self.downloader.open_track_audio(&track.track_id, priority),
        )
        .await
        .context("downloader response headers timed out")??;
        if download.content_length == 0
            || download.content_length > self.settings.max_track_size_bytes
        {
            anyhow::bail!(
                "declared track size {} is outside the allowed range",
                download.content_length
            );
        }
        let reservation = self
            .budget
            .reserve(download.content_length, priority, timeout)
            .await?;
        {
            *track
                ._reservation
                .lock()
                .expect("track reservation mutex poisoned") = Some(reservation);
            let mut state = track.state.lock().expect("track buffer mutex poisoned");
            state.bytes.reserve(download.content_length as usize);
            state.metadata = Some(TrackStreamMetadata {
                content_length: download.content_length,
                content_type: download.content_type.clone(),
            });
            state.extension = Some(download.extension.clone());
        }
        track.set_total_bytes(download.content_length);
        track.set_phase(ProxyJobPhase::Downloading);
        track.changed.notify_waiters();

        while let Some(chunk) = tokio::time::timeout(timeout, download.stream.next())
            .await
            .context("downloader made no progress")?
        {
            let chunk = chunk?;
            let mut state = track.state.lock().expect("track buffer mutex poisoned");
            if state.bytes.len().saturating_add(chunk.len()) > download.content_length as usize {
                anyhow::bail!("downloader sent more bytes than declared");
            }
            state.bytes.extend_from_slice(&chunk);
            drop(state);
            track.add_downloaded_bytes(chunk.len());
            track.changed.notify_waiters();
        }

        track.set_phase(ProxyJobPhase::Validating);
        {
            let mut state = track.state.lock().expect("track buffer mutex poisoned");
            if state.bytes.len() as u64 != download.content_length {
                anyhow::bail!(
                    "downloader length mismatch: expected {}, received {}",
                    download.content_length,
                    state.bytes.len()
                );
            }
            state.complete = true;
        }
        track.changed.notify_waiters();

        // Once validated, persist immediately. Retention is reconciled later
        // from durable listening history by the proxy cleanup job.
        drop(_slot);
        self.publish(&track, &download.extension).await?;
        track.finish(ProxyJobPhase::Completed, None);
        info!(track_id = %track.track_id, bytes = download.content_length, "Proxy track published");

        if priority != DownloadPriority::Prefetch {
            self.schedule_successor(&track.track_id);
        }
        Ok(())
    }

    async fn publish(&self, track: &InFlightTrack, extension: &str) -> Result<()> {
        let manager = self.media.upgrade().context("media manager stopped")?;
        track.set_phase(ProxyJobPhase::Saving);
        let stage = manager
            .stage(
                track.track_id.clone(),
                extension.to_owned(),
                super::Provenance::Proxy {
                    materialized_at: (track.status().started_at_ms / 1000) as i64,
                },
            )
            .await?;
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(stage.path())
            .await?;
        let content_length = track.metadata().await?.content_length as usize;
        let mut offset = 0;
        while offset < content_length {
            let chunk = {
                let state = track.state.lock().expect("track buffer mutex poisoned");
                let end = (offset + RESPONSE_CHUNK_SIZE).min(state.bytes.len());
                Bytes::copy_from_slice(&state.bytes[offset..end])
            };
            file.write_all(&chunk).await?;
            offset += chunk.len();
        }
        file.sync_all().await?;
        drop(file);
        track.set_phase(ProxyJobPhase::WaitingForPublication);
        manager.commit(stage).await?;
        Ok(())
    }

    fn schedule_successor(self: &Arc<Self>, track_id: &str) {
        if self.settings.max_prefetch_downloads == 0 {
            return;
        }
        let Some(track) = self.catalog.get_track(track_id).ok().flatten() else {
            return;
        };
        let Some(album) = self
            .catalog
            .get_resolved_album(&track.album_id)
            .ok()
            .flatten()
        else {
            return;
        };
        let mut tracks = album
            .discs
            .into_iter()
            .flat_map(|disc| disc.tracks)
            .collect::<Vec<_>>();
        tracks.sort_by_key(|candidate| (candidate.disc_number, candidate.track_number));
        let Some(index) = tracks.iter().position(|candidate| candidate.id == track_id) else {
            return;
        };
        let Some(next) = tracks.get(index + 1) else {
            return;
        };
        if next.availability != TrackAvailability::Available {
            self.get_or_start(&next.id, DownloadPriority::Prefetch);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;

    #[tokio::test]
    async fn range_stream_releases_bytes_as_the_buffer_grows() {
        let track = Arc::new(InFlightTrack::new(
            "track".into(),
            DownloadPriority::Foreground,
        ));
        {
            let mut state = track.state.lock().unwrap();
            state.metadata = Some(TrackStreamMetadata {
                content_length: 6,
                content_type: "audio/mpeg".into(),
            });
            state.bytes.extend_from_slice(b"abc");
        }
        let mut stream = track.clone().range_stream(1, 4);
        assert_eq!(
            stream.next().await.unwrap().unwrap(),
            Bytes::from_static(b"bc")
        );

        let producer = track.clone();
        tokio::spawn(async move {
            tokio::task::yield_now().await;
            let mut state = producer.state.lock().unwrap();
            state.bytes.extend_from_slice(b"def");
            state.complete = true;
            drop(state);
            producer.changed.notify_waiters();
        });
        assert_eq!(
            stream.next().await.unwrap().unwrap(),
            Bytes::from_static(b"de")
        );
        assert!(stream.next().await.is_none());
        assert_eq!(track.status().bytes_streamed, 4);
        drop(stream);
        assert_eq!(track.status().active_streams, 0);
    }

    #[tokio::test]
    async fn range_stream_propagates_download_failure() {
        let track = Arc::new(InFlightTrack::new(
            "track".into(),
            DownloadPriority::Foreground,
        ));
        let mut stream = track.clone().range_stream(0, 1);
        let producer = track.clone();
        tokio::spawn(async move {
            producer.state.lock().unwrap().error = Some("source failed".into());
            producer.changed.notify_waiters();
        });
        let error = stream.next().await.unwrap().unwrap_err();
        assert!(error.to_string().contains("source failed"));
    }

    #[test]
    fn streamed_ranges_count_unique_coverage_for_status() {
        let track = InFlightTrack::new("track".into(), DownloadPriority::Foreground);
        track.set_total_bytes(100);

        track.record_streamed_range(0, 30);
        track.record_streamed_range(20, 40);
        track.record_streamed_range(80, 90);

        assert_eq!(track.status().bytes_streamed, 50);
    }
}
