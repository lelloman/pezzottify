//! Serialized album queue fulfillment with durable retries and wake deadlines.
use crate::{
    album_worker_history::{self, History},
    download_manager::{DownloadContentType, DownloadManager, QueueItem, QueueStatus},
    downloader::{DownloadPriority, Downloader, DownloaderClient},
    ingestion::IngestionManager,
};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};
use tokio::io::AsyncWriteExt;

#[derive(Deserialize)]
struct MetadataAlbum {
    id: String,
    discs: Vec<MetadataDisc>,
}
#[derive(Deserialize)]
struct MetadataDisc {
    tracks: Vec<String>,
}
#[derive(Deserialize)]
struct MetadataTrack {
    id: String,
    album_id: String,
    name: String,
    number: i32,
    disc_number: i32,
}

#[derive(Serialize, Deserialize)]
struct Track {
    id: String,
    name: String,
    number: i32,
    disc_number: i32,
}
#[derive(Serialize, Deserialize)]
struct Active {
    item: QueueItem,
    run: i64,
    attempt: i64,
    token: i64,
    job: String,
    tracks: Option<Vec<Track>>,
    next: usize,
    retry: u32,
    retry_at: i64,
}
#[derive(Serialize, Deserialize, Default)]
struct WakeIntent {
    revision: i64,
    run_at: Option<String>,
}

pub(crate) struct Worker {
    manager: Arc<DownloadManager>,
    ingestion: Arc<IngestionManager>,
    history: History,
    downloader: DownloaderClient,
    metadata: reqwest::Client,
    active: Option<Active>,
    circuit_until: i64,
    consecutive_failures: u32,
}

fn content_error(error: &str) -> bool {
    [
        "is not available",
        "NOT_FOUND",
        "not found",
        "404 Not Found",
    ]
    .iter()
    .any(|needle| error.contains(needle))
}

impl Worker {
    pub(crate) fn new(
        manager: Arc<DownloadManager>,
        ingestion: Arc<IngestionManager>,
        history_path: PathBuf,
        registry: &crate::backup::DbRegistry,
        downloader_url: String,
    ) -> Result<Self> {
        let history = History::open(&history_path, registry)?;
        let active = history
            .state("active")?
            .map(|value| serde_json::from_str(&value))
            .transpose()?
            .flatten();
        let circuit_until = history
            .state("circuit_until")?
            .map(|value| value.parse())
            .transpose()?
            .unwrap_or(0);
        Ok(Self {
            manager,
            ingestion,
            history,
            downloader: DownloaderClient::new(downloader_url, 15),
            metadata: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()?,
            active,
            circuit_until,
            consecutive_failures: 0,
        })
    }

    fn save_active(&self) -> Result<()> {
        self.history
            .set_state("active", &serde_json::to_string(&self.active)?)
    }

    async fn publish(&self, deadline: Option<i64>) -> Result<()> {
        if !crate::power::enabled() {
            return Ok(());
        }
        let candidate = deadline
            .map(|at| {
                DateTime::from_timestamp(at, 0)
                    .context("Wake timestamp out of range")
                    .map(|value| value.to_rfc3339())
            })
            .transpose()?;
        let mut intent: WakeIntent = self
            .history
            .state("wakeup")?
            .map(|value| serde_json::from_str(&value))
            .transpose()?
            .unwrap_or_default();
        let remote = crate::power::schedule_revision("album_queue").await?;
        if intent.run_at != candidate || intent.revision == 0 || remote > intent.revision {
            intent.revision = intent
                .revision
                .max(remote)
                .checked_add(1)
                .context("Wake revision overflow")?;
            intent.run_at = candidate;
            self.history
                .set_state("wakeup", &serde_json::to_string(&intent)?)?;
        }
        crate::power::schedule("album_queue", intent.revision, intent.run_at).await
    }

    fn candidates(&self, now: i64) -> Result<(Vec<QueueItem>, Option<i64>)> {
        let attempts = self.history.attempts()?;
        let blocked = album_worker_history::blacklisted(&attempts);
        let quota = album_worker_history::quota_ready_at(&attempts, now * 1_000_000)? / 1_000_000;
        let mut ready = Vec::new();
        let mut future: Option<i64> = None;
        // Pagination prevents a large failed queue from hiding newer work.
        let mut offset = 0;
        loop {
            let page = self
                .manager
                .get_all_requests_filtered(None, true, true, 1000, offset)?;
            let count = page.len();
            for item in page {
                if item.content_type != DownloadContentType::Album
                    || !matches!(
                        item.status,
                        QueueStatus::Pending
                            | QueueStatus::InProgress
                            | QueueStatus::Failed
                            | QueueStatus::RetryWaiting
                    )
                    || blocked.contains(&item.content_id)
                {
                    continue;
                }
                let mut due = quota
                    .max(self.circuit_until)
                    .max(item.next_retry_at.unwrap_or(0));
                if let Some(at) =
                    album_worker_history::recent_completion(&attempts, &item.id, now * 1_000_000)
                {
                    due = due.max(at / 1_000_000 + 86401);
                }
                if let Some(at) = album_worker_history::quarantine_until(
                    &attempts,
                    &item.content_id,
                    now * 1_000_000,
                ) {
                    due = due.max(at / 1_000_000 + 1);
                }
                if due > now {
                    future = Some(future.map_or(due, |at| at.min(due)));
                } else {
                    ready.push(item);
                }
            }
            if count < 1000 {
                break;
            }
            offset += count;
        }
        ready.sort_by_key(|item| (item.priority as i32, item.created_at, item.id.clone()));
        Ok((ready, future))
    }

    async fn metadata(&self, item: &QueueItem) -> Result<Vec<Track>> {
        anyhow::ensure!(
            item.content_id.bytes().all(|c| c.is_ascii_alphanumeric()),
            "Invalid album ID"
        );
        let album: MetadataAlbum = self
            .metadata
            .get(format!(
                "{}/album/{}",
                self.downloader.base_url(),
                item.content_id
            ))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        anyhow::ensure!(album.id == item.content_id, "Album identity mismatch");
        let tracks: Vec<MetadataTrack> = self
            .metadata
            .get(format!(
                "{}/album/{}/tracks",
                self.downloader.base_url(),
                item.content_id
            ))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let mut canonical: HashMap<String, usize> = HashMap::new();
        for id in album.discs.into_iter().flat_map(|disc| disc.tracks) {
            *canonical.entry(id).or_default() += 1;
        }
        let mut actual: HashMap<String, usize> = HashMap::new();
        for track in &tracks {
            anyhow::ensure!(
                track.album_id == item.content_id
                    && !track.id.is_empty()
                    && track.id.bytes().all(|c| c.is_ascii_alphanumeric()),
                "Album track identity mismatch"
            );
            *actual.entry(track.id.clone()).or_default() += 1;
        }
        anyhow::ensure!(
            !tracks.is_empty() && canonical == actual,
            "Incomplete canonical album metadata"
        );
        Ok(tracks
            .into_iter()
            .map(|t| Track {
                id: t.id,
                name: t.name,
                number: t.number,
                disc_number: t.disc_number,
            })
            .collect())
    }

    async fn download(&self, track: &Track, dir: &std::path::Path) -> Result<()> {
        // Stable names allow a restart to reuse only atomically published files.
        for extension in ["mp3", "ogg", "flac", "m4a", "wav", "aac", "opus", "wma"] {
            let path = dir.join(format!("{}.{}", track.id, extension));
            if tokio::fs::metadata(&path).await.is_ok_and(|m| m.len() > 0) {
                return Ok(());
            }
        }
        tokio::time::timeout(Duration::from_secs(180), async {
            let mut audio = self
                .downloader
                .open_track_audio(&track.id, DownloadPriority::Normal)
                .await?;
            anyhow::ensure!(
                ["mp3", "ogg", "flac", "m4a", "wav", "aac", "opus", "wma"]
                    .contains(&audio.extension.as_str()),
                "Unsupported audio extension"
            );
            let partial = dir.join(format!("{}.part", track.id));
            let destination = dir.join(format!("{}.{}", track.id, audio.extension));
            let mut file = tokio::fs::File::create(&partial).await?;
            let mut size = 0u64;
            while let Some(chunk) = audio.stream.next().await {
                let chunk = chunk?;
                file.write_all(&chunk).await?;
                size += chunk.len() as u64;
            }
            anyhow::ensure!(
                size > 0 && size == audio.content_length,
                "Incomplete audio response"
            );
            file.sync_all().await?;
            drop(file);
            tokio::fs::rename(&partial, &destination).await?;
            // Persist the directory entry before advancing the durable cursor.
            tokio::fs::File::open(dir).await?.sync_all().await?;
            Ok::<(), anyhow::Error>(())
        })
        .await
        .context("Track download timed out")??;
        Ok(())
    }

    async fn advance(&mut self) -> Result<()> {
        let active = self.active.as_ref().context("No active album")?;
        if active.tracks.is_none() {
            let tracks = self.metadata(&active.item).await?;
            self.active.as_mut().unwrap().tracks = Some(tracks);
            self.save_active()?;
        }
        let active = self.active.as_ref().unwrap();
        let dir = self.ingestion.worker_directory(&active.job).await?;
        let tracks = active.tracks.as_ref().unwrap();
        if active.next < tracks.len() {
            let track = &tracks[active.next];
            match self.download(track, &dir).await {
                Ok(()) => {}
                Err(error) if content_error(&format!("{error:#}")) => {
                    tracing::warn!(track=%track.id,%error,"Unavailable album track left for ingestion review");
                }
                Err(error) => return Err(error),
            }
            let active = self.active.as_mut().unwrap();
            active.next += 1;
            active.retry = 0;
            active.retry_at = 0;
            self.save_active()?;
            return Ok(());
        }
        let user = active
            .item
            .requested_by_user_id
            .as_deref()
            .context("Album request has no owner")?;
        self.ingestion
            .ingest_download_directory(
                &active.job,
                user,
                &active.item.id,
                active
                    .item
                    .content_name
                    .as_deref()
                    .unwrap_or(&active.item.content_id),
            )
            .await?;
        self.history.finish(
            active.run,
            active.attempt,
            Ok(&active.job),
            tracks.len() as i64,
        )?;
        self.active = None;
        self.consecutive_failures = 0;
        self.save_active()?;
        Ok(())
    }

    async fn step(&mut self) -> Result<()> {
        let _inspection = crate::power::acquire(false).await?;
        let now = Utc::now().timestamp();
        if self.active.is_none() {
            let (ready, future) = self.candidates(now)?;
            let Some(item) = ready.into_iter().next() else {
                return self.publish(future).await;
            };
            let _demand = crate::power::acquire(true).await?;
            self.downloader.health_check().await?;
            let Some(token) = self
                .manager
                .start_external_attempt(&item.id, item.last_attempt_at)?
            else {
                return Ok(());
            };
            let (run, attempt) = self.history.begin(&item)?;
            self.active = Some(Active {
                item,
                run,
                attempt,
                token,
                job: uuid::Uuid::new_v4().to_string(),
                tracks: None,
                next: 0,
                retry: 0,
                retry_at: 0,
            });
            self.save_active()?;
        }
        let active = self.active.as_ref().unwrap();
        if !self
            .manager
            .external_attempt_is_current(&active.item.id, active.token)?
        {
            self.history.finish(
                active.run,
                active.attempt,
                Err("Queue claim superseded or cancelled"),
                0,
            )?;
            self.active = None;
            self.save_active()?;
            return self.publish(Some(now + 1)).await;
        }
        let due = self.active.as_ref().unwrap().retry_at;
        if due > now {
            return self.publish(Some(due)).await;
        }
        let _activity = crate::power::acquire(true).await?;
        match self.advance().await {
            Ok(()) => {}
            Err(error) => {
                let message = format!("{error:#}");
                let active = self.active.as_mut().unwrap();
                if active.retry < 4 && !content_error(&message) {
                    let delay = (15.0 * 1.5f64.powi(active.retry as i32)).ceil() as i64;
                    active.retry += 1;
                    active.retry_at = Utc::now().timestamp() + delay;
                    tracing::warn!(%error,retry=active.retry,delay,"Album work will retry without holding activity while waiting");
                    self.save_active()?;
                } else {
                    self.manager
                        .fail_external_attempt(&active.item.id, active.token, &message)?;
                    self.history.finish(
                        active.run,
                        active.attempt,
                        Err(&message),
                        active.tracks.as_ref().map_or(0, |t| t.len() as i64),
                    )?;
                    if !content_error(&message) {
                        self.consecutive_failures += 1;
                    }
                    self.active = None;
                    self.save_active()?;
                    if self.consecutive_failures >= 2 {
                        self.circuit_until = Utc::now().timestamp() + 1800;
                        self.history
                            .set_state("circuit_until", &self.circuit_until.to_string())?;
                        self.consecutive_failures = 0;
                        tracing::error!("Album downloader circuit paused after consecutive infrastructure failures");
                    }
                }
            }
        }
        let deadline = if let Some(active) = &self.active {
            Some(active.retry_at.max(Utc::now().timestamp() + 1))
        } else {
            let (ready, future) = self.candidates(Utc::now().timestamp())?;
            if ready.is_empty() {
                future
            } else {
                Some(Utc::now().timestamp() + 1)
            }
        };
        self.publish(deadline).await
    }

    pub(crate) async fn run(
        mut self,
        shutdown: simple_server::lifecycle::Shutdown,
    ) -> std::io::Result<()> {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! { biased; _=shutdown.requested()=>break, _=interval.tick()=>{} }
            match self.step().await {
                Ok(()) => crate::power::worker_synced(true).await,
                Err(error) => {
                    crate::power::worker_synced(false).await;
                    tracing::error!(%error,"Album queue reconciliation failed; suspend remains inhibited");
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        backup::DbRegistry,
        catalog_store::NullCatalogStore,
        config::DownloadManagerSettings,
        download_manager::{
            DownloadQueueStore, QueuePriority, RequestSource, SqliteDownloadQueueStore,
        },
        ingestion::{IngestionManagerConfig, SqliteIngestionStore},
        search::Fts5LevenshteinSearchVault,
    };
    use simple_server::{
        testing::TestServer,
        web::{body::Body, response::Response, routing::get, Json, Router},
    };
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn fixture(root: &std::path::Path, url: String) -> Worker {
        let registry = DbRegistry::new();
        let catalog = Arc::new(NullCatalogStore);
        let queue =
            Arc::new(SqliteDownloadQueueStore::new(root.join("queue.db"), &registry).unwrap());
        let mut item = QueueItem::new(
            "request".into(),
            DownloadContentType::Album,
            "album".into(),
            QueuePriority::User,
            RequestSource::User,
            8,
        );
        item.requested_by_user_id = Some("1".into());
        queue.enqueue(item).unwrap();
        let manager = Arc::new(DownloadManager::new(
            queue,
            catalog.clone(),
            root.join("media"),
            DownloadManagerSettings::default(),
        ));
        let search = Arc::new(
            Fts5LevenshteinSearchVault::new_lazy(&root.join("search.db"), &registry).unwrap(),
        );
        let ingestion = Arc::new(IngestionManager::new(
            Arc::new(SqliteIngestionStore::open(&root.join("ingestion.db"), &registry).unwrap()),
            catalog,
            search,
            IngestionManagerConfig {
                temp_dir: root.join("uploads"),
                media_dir: root.join("media"),
                ..Default::default()
            },
            Some(manager.clone()),
        ));
        Worker::new(manager, ingestion, root.join("history.db"), &registry, url).unwrap()
    }

    async fn downloader(incomplete: bool, count: Arc<AtomicUsize>) -> TestServer {
        let router=Router::new()
            .route("/health",get(||async {"ok"}))
            .route("/album/album",get(||async {Json(serde_json::json!({"id":"album","discs":[{"tracks":["track1","track2"]}]}))}))
            .route("/album/album/tracks",get(move ||async move {
                let mut tracks=vec![serde_json::json!({"id":"track1","album_id":"album","name":"one","number":1,"disc_number":1})];
                if !incomplete { tracks.push(serde_json::json!({"id":"track2","album_id":"album","name":"two","number":2,"disc_number":1})); }
                Json(tracks)
            }))
            .route("/track/{id}/audio",get(move || {
                let count=count.clone();
                async move {
                    count.fetch_add(1,Ordering::SeqCst);
                    Response::builder().header("Content-Type","audio/ogg")
                        .header("Content-Length","5").header("X-Pezzottify-Audio-Extension","ogg")
                        .body(Body::from("audio")).unwrap()
                }
            }));
        TestServer::tcp(router).await.unwrap()
    }

    #[tokio::test]
    async fn incomplete_metadata_never_downloads_and_retry_survives_restart() {
        let root = tempfile::TempDir::new().unwrap();
        let count = Arc::new(AtomicUsize::new(0));
        let downloader = downloader(true, count.clone()).await;
        let mut worker = fixture(root.path(), downloader.base_url().unwrap());
        worker.step().await.unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 0);
        let active = worker.active.as_ref().unwrap();
        assert_eq!(active.retry, 1);
        assert!(active.retry_at > Utc::now().timestamp());
        let manager = worker.manager.clone();
        let ingestion = worker.ingestion.clone();
        let retry_at = active.retry_at;
        drop(worker);
        let restored = Worker::new(
            manager,
            ingestion,
            root.path().join("history.db"),
            &DbRegistry::new(),
            downloader.base_url().unwrap(),
        )
        .unwrap();
        assert_eq!(restored.active.unwrap().retry_at, retry_at);
    }

    #[tokio::test]
    async fn resumed_album_keeps_claim_and_never_redownloads_finished_tracks() {
        let root = tempfile::TempDir::new().unwrap();
        let count = Arc::new(AtomicUsize::new(0));
        let downloader = downloader(false, count.clone()).await;
        let mut worker = fixture(root.path(), downloader.base_url().unwrap());
        worker.step().await.unwrap();
        assert_eq!(count.load(Ordering::SeqCst), 1);
        let active = worker.active.as_ref().unwrap();
        assert_eq!(active.next, 1);
        let token = active.token;
        let job = active.job.clone();
        let manager = worker.manager.clone();
        let ingestion = worker.ingestion.clone();
        drop(worker);
        let mut restored = Worker::new(
            manager,
            ingestion,
            root.path().join("history.db"),
            &DbRegistry::new(),
            downloader.base_url().unwrap(),
        )
        .unwrap();
        restored.step().await.unwrap();
        let active = restored.active.unwrap();
        assert_eq!(active.token, token);
        assert_eq!(active.job, job);
        assert_eq!(active.next, 2);
        assert_eq!(count.load(Ordering::SeqCst), 2);
        assert_eq!(
            std::fs::read(root.path().join("uploads").join(job).join("track1.ogg")).unwrap(),
            b"audio"
        );
    }
}
