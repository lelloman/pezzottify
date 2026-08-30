//! Shared, bounded, progressive materialization of missing catalog tracks.

use crate::catalog_store::{CatalogStore, TrackAvailability};
use crate::config::ProxyModeSettings;
use crate::downloader::{DownloadPriority, Downloader};
use crate::ingestion::FileHandler;
use crate::search::{HashedItemType, SearchVault};
use anyhow::{Context, Result};
use bytes::Bytes;
use futures::{Stream, StreamExt};
use std::collections::HashMap;
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::sync::{Notify, Semaphore};
use tracing::{debug, info, warn};

const RESPONSE_CHUNK_SIZE: usize = 64 * 1024;

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

pub struct InFlightTrack {
    track_id: String,
    state: Mutex<BufferState>,
    changed: Notify,
    _reservation: Mutex<Option<MemoryReservation>>,
}

impl InFlightTrack {
    fn new(track_id: String) -> Self {
        Self {
            track_id,
            state: Mutex::new(BufferState::default()),
            changed: Notify::new(),
            _reservation: Mutex::new(None),
        }
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
        Box::pin(futures::stream::unfold(
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
                            return Some((Ok(bytes), (track.clone(), next, end)));
                        }
                        Some(Err(error)) => return Some((Err(error), (track.clone(), end, end))),
                        None => notified.await,
                    }
                }
            },
        ))
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
    search: Arc<dyn SearchVault>,
    media_path: std::path::PathBuf,
    settings: ProxyModeSettings,
    jobs: Mutex<HashMap<String, Arc<InFlightTrack>>>,
    budget: Arc<MemoryBudget>,
    foreground_slots: Semaphore,
    prefetch_slots: Semaphore,
}

impl TrackMaterializer {
    pub fn new(
        downloader: Arc<dyn Downloader>,
        catalog: Arc<dyn CatalogStore>,
        search: Arc<dyn SearchVault>,
        media_path: std::path::PathBuf,
        settings: ProxyModeSettings,
    ) -> Arc<Self> {
        Arc::new(Self {
            downloader,
            catalog,
            search,
            media_path,
            budget: Arc::new(MemoryBudget {
                limit: settings.memory_budget_bytes,
                foreground_reserve: settings.max_track_size_bytes,
                used: Mutex::new(0),
                changed: Notify::new(),
            }),
            foreground_slots: Semaphore::new(settings.max_foreground_downloads),
            prefetch_slots: Semaphore::new(settings.max_prefetch_downloads),
            settings,
            jobs: Mutex::new(HashMap::new()),
        })
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
        let track = Arc::new(InFlightTrack::new(track_id.to_string()));
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
                let mut state = task_track
                    .state
                    .lock()
                    .expect("track buffer mutex poisoned");
                state.error = Some(error.to_string());
                drop(state);
                task_track.changed.notify_waiters();
            }
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
        let _slot = match priority {
            DownloadPriority::Prefetch => self.prefetch_slots.acquire().await?,
            _ => self.foreground_slots.acquire().await?,
        };
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
            track.changed.notify_waiters();
        }

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
        self.publish(&track, &download.extension).await?;
        info!(track_id = %track.track_id, bytes = download.content_length, "Proxy track published");

        if priority != DownloadPriority::Prefetch {
            self.schedule_successor(&track.track_id);
        }
        Ok(())
    }

    async fn publish(&self, track: &InFlightTrack, extension: &str) -> Result<()> {
        let (dir1, dir2) = FileHandler::shard_dirs(&track.track_id);
        let relative = format!("audio/{dir1}/{dir2}/{}.{}", track.track_id, extension);
        let destination = self.media_path.join(&relative);
        let parent = destination
            .parent()
            .context("audio destination has no parent")?;
        tokio::fs::create_dir_all(parent).await?;
        let temp = parent.join(format!(".{}.{}.part", track.track_id, uuid::Uuid::new_v4()));
        let mut file = tokio::fs::File::create(&temp).await?;
        let content_length = track.metadata().await?.content_length as usize;
        let write_result: Result<()> = async {
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
            Ok(())
        }
        .await;
        if let Err(error) = write_result {
            drop(file);
            let _ = tokio::fs::remove_file(&temp).await;
            return Err(error);
        }
        drop(file);
        if let Err(error) = tokio::fs::rename(&temp, &destination).await {
            let _ = tokio::fs::remove_file(&temp).await;
            return Err(error.into());
        }

        self.catalog
            .set_track_audio_uri(&track.track_id, &relative)?;
        let album_id = self
            .catalog
            .get_track_album_id(&track.track_id)
            .context("published track has no album")?;
        let album_availability = self.catalog.recompute_album_availability(&album_id)?;
        self.search.update_availability(&[
            (track.track_id.clone(), HashedItemType::Track, true),
            (
                album_id.clone(),
                HashedItemType::Album,
                album_availability != crate::catalog_store::AlbumAvailability::Missing,
            ),
        ]);
        for artist_id in self.catalog.get_album_artist_ids(&album_id)? {
            match self.catalog.recompute_artist_availability(&artist_id) {
                Ok(available) => self.search.update_availability(&[(
                    artist_id,
                    HashedItemType::Artist,
                    available,
                )]),
                Err(error) => debug!(%error, "Failed to recompute proxy artist availability"),
            }
        }
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
        let track = Arc::new(InFlightTrack::new("track".into()));
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
    }

    #[tokio::test]
    async fn range_stream_propagates_download_failure() {
        let track = Arc::new(InFlightTrack::new("track".into()));
        let mut stream = track.clone().range_stream(0, 1);
        let producer = track.clone();
        tokio::spawn(async move {
            producer.state.lock().unwrap().error = Some("source failed".into());
            producer.changed.notify_waiters();
        });
        let error = stream.next().await.unwrap().unwrap_err();
        assert!(error.to_string().contains("source failed"));
    }
}
