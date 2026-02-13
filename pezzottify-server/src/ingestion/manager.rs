//! Ingestion Manager - orchestrates album-first ingestion workflows.
//!
//! Album-first workflow:
//! 1. User uploads zip → extract audio files → create job + file records
//! 2. ANALYZING: Probe each file for audio metadata and embedded tags
//! 3. IDENTIFYING_ALBUM: Agent analyzes collective metadata to identify album
//! 4. AWAITING_REVIEW (if needed): Human confirms album match
//! 5. MAPPING_TRACKS: Map each file to a track in the album
//! 6. CONVERTING: Convert each file to OGG Vorbis
//! 7. COMPLETED: All done

use super::converter::{convert_to_ogg, probe_audio_file};
use super::file_handler::{FileHandler, FileHandlerError};
use super::fingerprint::{
    compare_durations, match_album_with_fallbacks, FingerprintConfig, FingerprintMatchResult,
    ScoredCandidate,
};
use super::models::{
    AlbumMetadataSummary, ConversionReason, IngestionContextType, IngestionFile, IngestionJob,
    IngestionJobStatus, IngestionMatchSource, ReviewOption, TicketType, UploadType,
};
use super::store::IngestionStore;
use crate::catalog_store::CatalogStore;
use crate::search::{HashedItemType, SearchVault};
use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use tracing::{debug, error, info, warn};

/// Minimal queue item info needed by IngestionManager.
#[derive(Debug, Clone)]
pub struct QueueItemInfo {
    /// Queue item ID
    pub id: String,
    /// Content ID (album ID for album downloads)
    pub content_id: String,
    /// Content name (album name)
    pub content_name: Option<String>,
    /// Artist name
    pub artist_name: Option<String>,
    /// User who requested this download
    pub requested_by_user_id: Option<String>,
}

/// Info about a completed download request (returned by auto-complete).
#[derive(Debug, Clone)]
pub struct CompletedRequestInfo {
    /// Queue item ID
    pub id: String,
    /// User who requested this download
    pub requested_by_user_id: Option<String>,
}

/// Trait for DownloadManager operations needed by IngestionManager.
#[cfg_attr(feature = "mock", mockall::automock)]
pub trait DownloadManagerTrait: Send + Sync {
    /// Get queue item info by ID.
    fn get_queue_item(&self, item_id: &str) -> Result<Option<QueueItemInfo>>;

    /// Mark a download request as completed.
    fn mark_request_completed(
        &self,
        item_id: &str,
        bytes_downloaded: u64,
        duration_ms: i64,
    ) -> Result<()>;

    /// Mark a download request as in-progress (prevents re-download by cron).
    fn mark_request_in_progress(&self, item_id: &str) -> Result<()>;

    /// Mark a download request as failed (e.g., when ingestion fails).
    fn mark_request_failed(&self, item_id: &str, error_message: &str) -> Result<()>;

    /// Complete all pending download requests for an album.
    /// Returns info about completed requests (including requesting user IDs).
    fn complete_requests_for_album(
        &self,
        album_id: &str,
        bytes_downloaded: u64,
        duration_ms: i64,
    ) -> Result<Vec<CompletedRequestInfo>>;
}

/// Errors that can occur during ingestion.
#[derive(Debug, Error)]
pub enum IngestionError {
    #[error("Store error: {0}")]
    Store(#[from] anyhow::Error),

    #[error("File handling error: {0}")]
    FileHandler(#[from] FileHandlerError),

    #[error("Conversion error: {0}")]
    Conversion(#[from] super::converter::ConversionError),

    #[error("Job not found: {0}")]
    JobNotFound(String),

    #[error("Invalid job state: expected {expected}, got {actual}")]
    InvalidState { expected: String, actual: String },

    #[error("No files in upload")]
    NoFiles,

    #[error("Album not matched")]
    AlbumNotMatched,
}

/// Album candidate with track information for scoring.
#[derive(Debug, Clone)]
struct AlbumCandidate {
    id: String,
    name: String,
    artist_name: String,
    track_count: i32,
    total_duration_ms: i64,
    track_titles: Vec<String>,
}

/// Album candidate info returned from job details query.
#[derive(Debug, Clone)]
pub struct AlbumCandidateInfo {
    /// Album ID.
    pub id: String,
    /// Album name.
    pub name: String,
    /// Artist name.
    pub artist_name: String,
    /// Track count.
    pub track_count: i32,
    /// Match score (0.0 - 1.0).
    pub score: f32,
    /// Duration delta in ms.
    pub delta_ms: i64,
}

/// Result of processing an upload (may create multiple jobs for collections).
#[derive(Debug, Clone)]
pub struct UploadResult {
    /// Upload session ID (groups jobs from same upload).
    pub session_id: String,
    /// Detected upload type.
    pub upload_type: UploadType,
    /// Created job IDs.
    pub job_ids: Vec<String>,
    /// Number of albums detected (for collections).
    pub album_count: usize,
}

/// Configuration for the IngestionManager.
#[derive(Clone)]
pub struct IngestionManagerConfig {
    /// Directory for temporary files.
    pub temp_dir: PathBuf,
    /// Directory for media output files.
    pub media_dir: PathBuf,
    /// Maximum file size in bytes.
    pub max_file_size: u64,
    /// Target bitrate for audio conversion (kbps).
    pub target_bitrate: u32,
    /// Acceptable bitrate range (± this value from target).
    pub bitrate_tolerance: u32,
    /// Maximum LLM iterations per job.
    pub max_iterations: usize,
    /// Confidence threshold for auto-matching (0.0 - 1.0).
    pub auto_match_threshold: f32,
}

impl Default for IngestionManagerConfig {
    fn default() -> Self {
        Self {
            temp_dir: PathBuf::from("/tmp/pezzottify-ingestion"),
            media_dir: PathBuf::from("media"),
            max_file_size: 500 * 1024 * 1024, // 500 MB for zip files
            target_bitrate: 320,
            bitrate_tolerance: 50,
            max_iterations: 20,
            auto_match_threshold: 0.85,
        }
    }
}

/// Parameters for creating an ingestion job.
#[derive(Debug)]
struct JobCreationParams<'a> {
    /// User ID creating the job
    user_id: &'a str,
    /// Job name
    name: &'a str,
    /// Total size in bytes
    total_size: i64,
    /// Directory containing audio files
    dir: &'a Path,
    /// Upload session ID (if from upload)
    session_id: Option<String>,
    /// Upload type (track, album, collection)
    upload_type: UploadType,
    /// Context type (manual, download, etc.)
    context_type: IngestionContextType,
    /// Context ID (e.g., download queue item ID)
    context_id: Option<String>,
}

/// Manages the album-first ingestion workflow.
pub struct IngestionManager {
    store: Arc<dyn IngestionStore>,
    catalog: Arc<dyn CatalogStore>,
    search: Arc<dyn SearchVault>,
    file_handler: FileHandler,
    config: IngestionManagerConfig,
    download_manager: Option<Arc<dyn DownloadManagerTrait>>,
    notifier: Option<Arc<super::notifier::IngestionNotifier>>,
    notification_service: Option<Arc<crate::notifications::NotificationService>>,
}

impl IngestionManager {
    /// Create a new IngestionManager.
    pub fn new(
        store: Arc<dyn IngestionStore>,
        catalog: Arc<dyn CatalogStore>,
        search: Arc<dyn SearchVault>,
        config: IngestionManagerConfig,
        download_manager: Option<Arc<dyn DownloadManagerTrait>>,
    ) -> Self {
        let file_handler = FileHandler::new(&config.temp_dir, config.max_file_size);

        Self {
            store,
            catalog,
            search,
            file_handler,
            config,
            download_manager,
            notifier: None,
            notification_service: None,
        }
    }

    /// Set the notifier for WebSocket updates.
    pub fn with_notifier(mut self, notifier: Arc<super::notifier::IngestionNotifier>) -> Self {
        self.notifier = Some(notifier);
        self
    }

    /// Set the notification service for download completion notifications.
    pub fn with_notification_service(
        mut self,
        service: Arc<crate::notifications::NotificationService>,
    ) -> Self {
        self.notification_service = Some(service);
        self
    }

    /// Initialize the manager (creates temp directory, etc.).
    pub async fn init(&self) -> Result<()> {
        self.file_handler.init().await?;
        Ok(())
    }

    /// Send a download-completed notification to a user.
    /// All errors are logged as warnings, never propagated.
    async fn send_download_notification(
        &self,
        user_id_str: &str,
        request_id: &str,
        album_id: &str,
        album_name: &str,
        artist_name: &str,
    ) {
        let notification_service = match &self.notification_service {
            Some(svc) => svc,
            None => return,
        };

        let user_id = match user_id_str.parse::<usize>() {
            Ok(id) => id,
            Err(_) => {
                warn!(
                    "Cannot send download notification: failed to parse user_id '{}'",
                    user_id_str
                );
                return;
            }
        };

        let image_id = match self.catalog.get_album_image_url(album_id) {
            Ok(Some(_)) => Some(album_id.to_string()),
            Ok(None) => None,
            Err(e) => {
                warn!("Failed to get album image for notification: {}", e);
                None
            }
        };

        let data = crate::notifications::DownloadCompletedData {
            album_id: album_id.to_string(),
            album_name: album_name.to_string(),
            artist_name: artist_name.to_string(),
            image_id,
            request_id: request_id.to_string(),
        };

        let data_json = match serde_json::to_value(&data) {
            Ok(v) => v,
            Err(e) => {
                warn!("Failed to serialize notification data: {}", e);
                return;
            }
        };

        if let Err(e) = notification_service
            .create_notification(
                user_id,
                crate::notifications::NotificationType::DownloadCompleted,
                format!("{} is ready", album_name),
                Some(format!("by {}", artist_name)),
                data_json,
            )
            .await
        {
            warn!(
                "Failed to create download notification for user {}: {}",
                user_id, e
            );
        }
    }

    /// Mark a job as failed, clean up temp files, and notify.
    ///
    /// This is the centralized failure handling for ingestion jobs. It:
    /// 1. Sets the job status to Failed
    /// 2. Records the error message
    /// 3. Sets the completed_at timestamp
    /// 4. Updates the job in the store
    /// 5. Cleans up temporary files
    /// 6. Notifies via WebSocket (if notifier is configured)
    async fn fail_job_with_cleanup(
        &self,
        job: &mut IngestionJob,
        error_message: &str,
    ) -> Result<(), IngestionError> {
        job.status = IngestionJobStatus::Failed;
        job.error_message = Some(error_message.to_string());
        job.completed_at = Some(chrono::Utc::now().timestamp_millis());
        self.store.update_job(job)?;

        // Clean up temp files
        if let Err(e) = self.file_handler.cleanup_job(&job.id).await {
            warn!("Failed to cleanup temp files for job {}: {}", job.id, e);
        }

        // Notify failure
        if let Some(notifier) = &self.notifier {
            notifier.notify_failed(job, error_message).await;
        }

        // If this job is from a download request, mark the queue item as failed
        if let (Some(IngestionContextType::DownloadRequest), Some(context_id)) =
            (job.context_type, &job.context_id)
        {
            if let Some(dm) = &self.download_manager {
                if let Err(e) = dm.mark_request_failed(context_id, error_message) {
                    warn!(
                        "Failed to mark download request {} as failed: {}",
                        context_id, e
                    );
                }
            }
        }

        Ok(())
    }

    // =========================================================================
    // Job Creation
    // =========================================================================

    /// Create a new ingestion job from uploaded file bytes (zip or single audio file).
    pub async fn create_job(
        &self,
        user_id: &str,
        filename: &str,
        data: &[u8],
        context_type: IngestionContextType,
        context_id: Option<String>,
    ) -> Result<String, IngestionError> {
        let job_id = uuid::Uuid::new_v4().to_string();
        let total_size = data.len() as i64;

        // Save uploaded file to temp storage
        let temp_path = self
            .file_handler
            .save_upload(&job_id, filename, data)
            .await?;

        // Extract audio files if it's a zip, otherwise use the single file
        let audio_files = if FileHandler::is_zip(filename) {
            self.file_handler.extract_zip(&job_id, &temp_path).await?
        } else if FileHandler::is_supported_audio(filename) {
            vec![temp_path.clone()]
        } else {
            return Err(IngestionError::FileHandler(
                FileHandlerError::UnsupportedFileType(filename.to_string()),
            ));
        };

        if audio_files.is_empty() {
            return Err(IngestionError::NoFiles);
        }

        let file_count = audio_files.len() as i32;

        // Create job record
        let job = IngestionJob::new(&job_id, user_id, filename, total_size, file_count)
            .with_context(context_type, context_id);

        self.store.create_job(&job)?;

        // Create file records for each audio file
        for audio_path in &audio_files {
            let file_id = uuid::Uuid::new_v4().to_string();
            let file_name = audio_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");
            let file_size = tokio::fs::metadata(&audio_path)
                .await
                .map(|m| m.len() as i64)
                .unwrap_or(0);

            let file = IngestionFile::new(
                &file_id,
                &job_id,
                file_name,
                file_size,
                audio_path.to_string_lossy().to_string(),
            );

            self.store.create_file(&file)?;
        }

        info!(
            "Created ingestion job {} for user {} with {} files from {}",
            job_id, user_id, file_count, filename
        );

        Ok(job_id)
    }

    /// Process an upload with automatic type detection and fingerprint matching.
    ///
    /// This is the main entry point for the redesigned ingestion flow:
    /// 1. Extracts files from upload
    /// 2. Detects upload type (Track, Album, Collection)
    /// 3. For collections: creates separate jobs per album
    /// 4. Runs duration fingerprint matching
    /// 5. Creates tickets based on match quality
    pub async fn process_upload(
        &self,
        user_id: &str,
        filename: &str,
        data: &[u8],
        context_type: IngestionContextType,
        context_id: Option<String>,
    ) -> Result<UploadResult, IngestionError> {
        let session_id = uuid::Uuid::new_v4().to_string();
        let total_size = data.len() as i64;

        // Create a temp session directory
        let session_dir = self.file_handler.create_job_dir(&session_id).await?;

        // Save uploaded file
        let temp_path = self
            .file_handler
            .save_upload(&session_id, filename, data)
            .await?;

        // Extract if zip
        let extract_dir = if FileHandler::is_zip(filename) {
            let audio_files = self
                .file_handler
                .extract_zip(&session_id, &temp_path)
                .await?;
            if audio_files.is_empty() {
                return Err(IngestionError::NoFiles);
            }
            session_dir.join("extracted")
        } else if FileHandler::is_supported_audio(filename) {
            // Single file - just use the session dir
            session_dir.clone()
        } else {
            return Err(IngestionError::FileHandler(
                FileHandlerError::UnsupportedFileType(filename.to_string()),
            ));
        };

        // Detect upload type
        let upload_type = self.file_handler.detect_upload_type(&extract_dir).await?;

        info!(
            session_id = %session_id,
            upload_type = ?upload_type,
            "Detected upload type"
        );

        // Save context_id for later use (it gets moved into JobCreationParams)
        let saved_context_id = context_id.clone();

        // Create jobs based on upload type
        let job_ids = match upload_type {
            UploadType::Track => {
                // Single track - create one job
                let job_id = self
                    .create_job_internal(JobCreationParams {
                        user_id,
                        name: filename,
                        total_size,
                        dir: &extract_dir,
                        session_id: Some(session_id.clone()),
                        upload_type,
                        context_type,
                        context_id,
                    })
                    .await?;
                vec![job_id]
            }
            UploadType::Album => {
                // Single album - create one job
                let job_id = self
                    .create_job_internal(JobCreationParams {
                        user_id,
                        name: filename,
                        total_size,
                        dir: &extract_dir,
                        session_id: Some(session_id.clone()),
                        upload_type,
                        context_type,
                        context_id,
                    })
                    .await?;
                vec![job_id]
            }
            UploadType::Collection => {
                // Collection - create one job per album directory
                let albums = self.file_handler.group_files_by_album(&extract_dir).await?;
                let mut job_ids = Vec::with_capacity(albums.len());

                for (album_dir, _files) in &albums {
                    let album_name = album_dir
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(filename);

                    let job_id = self
                        .create_job_internal(JobCreationParams {
                            user_id,
                            name: album_name,
                            total_size: 0, // Individual album size not tracked
                            dir: album_dir,
                            session_id: Some(session_id.clone()),
                            upload_type: UploadType::Album, // Each sub-job is an album
                            context_type,
                            context_id: context_id.clone(),
                        })
                        .await?;
                    job_ids.push(job_id);
                }

                job_ids
            }
        };

        let album_count = job_ids.len();

        // If this upload is from a download request, mark the queue item as IN_PROGRESS
        // so the cron downloader won't re-download the same content.
        if context_type == IngestionContextType::DownloadRequest {
            if let Some(ref ctx_id) = saved_context_id {
                if let Some(dm) = &self.download_manager {
                    if let Err(e) = dm.mark_request_in_progress(ctx_id) {
                        warn!(
                            "Failed to mark download request {} as in-progress: {}",
                            ctx_id, e
                        );
                    }
                }
            }
        }

        info!(
            session_id = %session_id,
            job_count = album_count,
            "Created ingestion jobs"
        );

        Ok(UploadResult {
            session_id,
            upload_type,
            job_ids,
            album_count,
        })
    }

    /// Internal helper to create a job from a directory of audio files.
    async fn create_job_internal(
        &self,
        params: JobCreationParams<'_>,
    ) -> Result<String, IngestionError> {
        let job_id = uuid::Uuid::new_v4().to_string();

        // Get audio files
        let audio_files = self
            .file_handler
            .list_audio_files_recursive(params.dir)
            .await?;
        if audio_files.is_empty() {
            return Err(IngestionError::NoFiles);
        }

        let file_count = audio_files.len() as i32;

        // Create job record with upload info
        let job = IngestionJob::new(
            &job_id,
            params.user_id,
            params.name,
            params.total_size,
            file_count,
        )
        .with_context(params.context_type, params.context_id)
        .with_upload_info(params.session_id, params.upload_type);

        self.store.create_job(&job)?;

        // Create file records
        for audio_path in &audio_files {
            let file_id = uuid::Uuid::new_v4().to_string();
            let file_name = audio_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");
            let file_size = tokio::fs::metadata(audio_path)
                .await
                .map(|m| m.len() as i64)
                .unwrap_or(0);

            let file = IngestionFile::new(
                &file_id,
                &job_id,
                file_name,
                file_size,
                audio_path.to_string_lossy().to_string(),
            );

            self.store.create_file(&file)?;
        }

        info!(
            job_id = %job_id,
            file_count = file_count,
            upload_type = ?params.upload_type,
            "Created ingestion job"
        );

        Ok(job_id)
    }

    /// Run fingerprint matching for a job and update its ticket type.
    pub async fn run_fingerprint_match(
        &self,
        job_id: &str,
    ) -> Result<FingerprintMatchResult, IngestionError> {
        let mut job = self
            .store
            .get_job(job_id)?
            .ok_or_else(|| IngestionError::JobNotFound(job_id.to_string()))?;

        // Get all files and their durations
        let files = self.store.get_files_for_job(job_id)?;

        // Extract durations (need to analyze if not already done)
        let mut durations: Vec<(i32, i64)> = Vec::with_capacity(files.len());

        for file in &files {
            // Get track number and duration
            let track_num = file.tag_track_num.unwrap_or(0);
            let duration = match file.duration_ms {
                Some(d) => d,
                None => {
                    // Need to probe the file
                    let path = Path::new(&file.temp_file_path);
                    let metadata = probe_audio_file(path).await?;
                    metadata.duration_ms as i64
                }
            };
            durations.push((track_num, duration));
        }

        // Sort by track number to ensure correct order
        durations.sort_by_key(|(track_num, _)| *track_num);
        let ordered_durations: Vec<i64> = durations.into_iter().map(|(_, d)| d).collect();

        // Run fingerprint matching
        let config = FingerprintConfig::default();
        let result =
            match_album_with_fallbacks(&ordered_durations, self.catalog.as_ref(), &config)?;

        // Update job with fingerprint results
        job.ticket_type = Some(result.ticket_type);
        job.match_score = Some(result.match_score);
        job.match_delta_ms = Some(result.total_delta_ms);

        if let Some(ref album) = result.matched_album {
            job.matched_album_id = Some(album.id.clone());
            job.match_confidence = Some(result.match_score);
            job.match_source = Some(IngestionMatchSource::Fingerprint);

            // Update detected metadata from the matched album
            job.detected_album = Some(album.name.clone());
            job.detected_artist = Some(album.artist_name.clone());
        }

        // Update status based on ticket type
        match result.ticket_type {
            TicketType::Success => {
                // Auto-matched, proceed to track mapping
                job.status = IngestionJobStatus::MappingTracks;
            }
            TicketType::Review => {
                // Needs human review
                job.status = IngestionJobStatus::AwaitingReview;
                // Create review item with top candidates
                self.create_fingerprint_review(&job, &result.candidates)?;
            }
            TicketType::Failure => {
                // No match - needs manual resolution
                job.status = IngestionJobStatus::AwaitingReview;
                self.create_failure_review(&job)?;
            }
        }

        self.store.update_job(&job)?;

        info!(
            job_id = %job_id,
            ticket_type = ?result.ticket_type,
            match_score = result.match_score,
            delta_ms = result.total_delta_ms,
            "Fingerprint matching complete"
        );

        Ok(result)
    }

    /// Create a review item for fingerprint match candidates.
    fn create_fingerprint_review(
        &self,
        job: &IngestionJob,
        candidates: &[ScoredCandidate],
    ) -> Result<(), IngestionError> {
        let options: Vec<ReviewOption> = candidates
            .iter()
            .map(|c| ReviewOption {
                id: format!("album:{}", c.album.id),
                label: format!("{} - {}", c.album.artist_name, c.album.name),
                description: Some(format!(
                    "Match: {:.0}%, Delta: {}ms, {} tracks",
                    c.score * 100.0,
                    c.delta_ms,
                    c.album.track_count
                )),
            })
            .collect();

        let options_json = serde_json::to_string(&options).unwrap_or_default();

        self.store.create_review_item(
            &job.id,
            "Multiple album candidates found. Please select the correct album:",
            &options_json,
        )?;

        Ok(())
    }

    /// Create a review item for failed fingerprint match.
    fn create_failure_review(&self, job: &IngestionJob) -> Result<(), IngestionError> {
        let options = vec![
            ReviewOption {
                id: "search".to_string(),
                label: "Search manually".to_string(),
                description: Some("Search the catalog for the correct album".to_string()),
            },
            ReviewOption {
                id: "dismiss".to_string(),
                label: "Dismiss upload".to_string(),
                description: Some("Reject this upload - album not in catalog".to_string()),
            },
        ];

        let options_json = serde_json::to_string(&options).unwrap_or_default();

        self.store.create_review_item(
            &job.id,
            &format!(
                "No matching album found for '{}'. Would you like to search manually?",
                job.original_filename
            ),
            &options_json,
        )?;

        Ok(())
    }

    /// Extract ordered track durations from ingestion files for a job.
    ///
    /// Returns durations in ms sorted by track number when available,
    /// falling back to filename sort when embedded tags are missing.
    async fn extract_ordered_durations(&self, job_id: &str) -> Result<Vec<i64>, IngestionError> {
        let files = self.store.get_files_for_job(job_id)?;

        let has_track_nums = files.iter().any(|f| f.tag_track_num.is_some());

        let mut entries: Vec<(i32, String, i64)> = Vec::with_capacity(files.len());

        for file in &files {
            let track_num = file.tag_track_num.unwrap_or(0);
            let duration = match file.duration_ms {
                Some(d) => d,
                None => {
                    let path = Path::new(&file.temp_file_path);
                    let metadata = probe_audio_file(path).await?;
                    metadata.duration_ms as i64
                }
            };
            entries.push((track_num, file.filename.clone(), duration));
        }

        if has_track_nums {
            entries.sort_by_key(|(track_num, _, _)| *track_num);
        } else {
            entries.sort_by(|(_, name_a, _), (_, name_b, _)| name_a.cmp(name_b));
        }

        Ok(entries.into_iter().map(|(_, _, d)| d).collect())
    }

    // =========================================================================
    // Job Queries
    // =========================================================================

    /// Get a job by ID.
    pub fn get_job(&self, job_id: &str) -> Result<Option<IngestionJob>, IngestionError> {
        Ok(self.store.get_job(job_id)?)
    }

    /// Get files for a job.
    pub fn get_files(&self, job_id: &str) -> Result<Vec<IngestionFile>, IngestionError> {
        Ok(self.store.get_files_for_job(job_id)?)
    }

    /// List jobs for a user.
    pub fn list_user_jobs(
        &self,
        user_id: &str,
        limit: usize,
    ) -> Result<Vec<IngestionJob>, IngestionError> {
        Ok(self.store.list_jobs_by_user(user_id, limit)?)
    }

    /// List all jobs (for admin).
    pub fn list_all_jobs(&self, limit: usize) -> Result<Vec<IngestionJob>, IngestionError> {
        Ok(self.store.list_all_jobs(limit)?)
    }

    /// Get detailed job information including candidates and review.
    ///
    /// Returns (candidates, review) where candidates are parsed from the review options
    /// if the job is awaiting review.
    pub fn get_job_details(
        &self,
        job_id: &str,
    ) -> Result<
        (
            Vec<AlbumCandidateInfo>,
            Option<super::models::ReviewQueueItem>,
        ),
        IngestionError,
    > {
        let review = self.store.get_review_item(job_id)?;

        let mut candidates = Vec::new();

        // Parse candidates from review options if available
        if let Some(ref review_item) = review {
            // Only parse if this is a pending review (not resolved)
            if review_item.resolved_at.is_none() {
                if let Ok(options) =
                    serde_json::from_str::<Vec<super::models::ReviewOption>>(&review_item.options)
                {
                    for opt in options {
                        // Parse album candidates from options like "album:abc123"
                        if opt.id.starts_with("album:") {
                            let album_id = opt.id.trim_start_matches("album:");

                            // Try to extract info from the option label/description
                            // Format: "Artist - Album (XX%, N tracks)"
                            let (score, track_count, delta_ms) =
                                parse_option_metadata(&opt.label, opt.description.as_deref());

                            // Try to get album name and artist from catalog
                            let (name, artist_name) =
                                if let Some(candidate) = self.build_album_candidate(album_id) {
                                    (candidate.name, candidate.artist_name)
                                } else {
                                    // Fallback: parse from label
                                    let parts: Vec<&str> = opt.label.splitn(2, " - ").collect();
                                    if parts.len() == 2 {
                                        (
                                            parts[1].split(" (").next().unwrap_or("").to_string(),
                                            parts[0].to_string(),
                                        )
                                    } else {
                                        (opt.label.clone(), "Unknown".to_string())
                                    }
                                };

                            candidates.push(AlbumCandidateInfo {
                                id: album_id.to_string(),
                                name,
                                artist_name,
                                track_count,
                                score,
                                delta_ms,
                            });
                        }
                    }
                }
            }
        }

        Ok((candidates, review))
    }

    // =========================================================================
    // Phase 1: Analyze Files
    // =========================================================================

    /// Analyze all files in a job - extract audio metadata and embedded tags.
    pub async fn analyze_job(&self, job_id: &str) -> Result<(), IngestionError> {
        let mut job = self
            .store
            .get_job(job_id)?
            .ok_or_else(|| IngestionError::JobNotFound(job_id.to_string()))?;

        if job.status != IngestionJobStatus::Pending {
            return Err(IngestionError::InvalidState {
                expected: "PENDING".to_string(),
                actual: job.status.as_str().to_string(),
            });
        }

        job.status = IngestionJobStatus::Analyzing;
        job.started_at = Some(chrono::Utc::now().timestamp_millis());
        self.store.update_job(&job)?;

        let files = self.store.get_files_for_job(job_id)?;
        let total_files = files.len();

        for (idx, mut file) in files.into_iter().enumerate() {
            // Notify progress
            if let Some(notifier) = &self.notifier {
                let progress = ((idx as f32 / total_files as f32) * 100.0) as u8;
                notifier
                    .notify_progress(&job, "analyzing", progress, idx as u32)
                    .await;
            }

            // Probe audio metadata
            let path = Path::new(&file.temp_file_path);
            match probe_audio_file(path).await {
                Ok(metadata) => {
                    file.duration_ms = Some(metadata.duration_ms);
                    file.codec = Some(metadata.codec);
                    file.bitrate = metadata.bitrate;
                    file.sample_rate = metadata.sample_rate;

                    // Determine if conversion is needed based on bitrate
                    file.conversion_reason = Some(self.determine_conversion_need(
                        file.bitrate,
                        &file.codec,
                        Path::new(&file.temp_file_path),
                    ));
                }
                Err(e) => {
                    warn!("Failed to probe {}: {}", file.filename, e);
                    file.error_message = Some(format!("Probe failed: {}", e));
                }
            }

            // Extract embedded tags using ffprobe
            if let Ok(tags) = self.extract_tags(path).await {
                file.tag_artist = tags.get("artist").cloned();
                file.tag_album = tags.get("album").cloned();
                file.tag_title = tags.get("title").cloned();
                file.tag_track_num = tags
                    .get("track")
                    .and_then(|t| t.split('/').next())
                    .and_then(|t| t.parse().ok());
                file.tag_track_total = tags
                    .get("track")
                    .and_then(|t| t.split('/').nth(1))
                    .and_then(|t| t.parse().ok());
                file.tag_disc_num = tags
                    .get("disc")
                    .and_then(|d| d.split('/').next())
                    .and_then(|d| d.parse().ok());
                file.tag_year = tags
                    .get("date")
                    .and_then(|d| d.get(..4))
                    .and_then(|y| y.parse().ok());
            }

            self.store.update_file(&file)?;
        }

        // Notify 100% completion of analyzing phase
        if let Some(notifier) = &self.notifier {
            notifier
                .notify_progress(&job, "analyzing", 100, total_files as u32)
                .await;
        }

        // Count probe failures — fail early if no files could be probed
        let files_after = self.store.get_files_for_job(job_id)?;
        let probed_count = files_after
            .iter()
            .filter(|f| f.duration_ms.is_some())
            .count();
        let failed_count = files_after.len() - probed_count;

        if probed_count == 0 {
            let error_msg = format!(
                "All {} audio files failed to probe — files may be corrupted or in an unsupported format",
                failed_count
            );
            self.fail_job_with_cleanup(&mut job, &error_msg).await?;

            return Err(IngestionError::Store(anyhow::anyhow!(
                "All files failed audio probe"
            )));
        }

        if failed_count > 0 {
            warn!(
                "Job {} — {}/{} files failed to probe",
                job_id,
                failed_count,
                files_after.len()
            );
        }

        // Aggregate detected metadata
        let summary = self.build_metadata_summary(job_id)?;
        job.detected_artist = summary.artist;
        job.detected_album = summary.album;
        job.detected_year = summary.year;

        // Check for low bitrate files before continuing
        if self.check_low_bitrate_files(job_id, &mut job).await? {
            return Ok(()); // Job is now in AwaitingReview
        }

        // No low bitrate issues, continue to identification
        job.status = IngestionJobStatus::IdentifyingAlbum;
        self.store.update_job(&job)?;

        // Verify files still exist after analysis
        let files_for_verify = self.store.get_files_for_job(job_id)?;
        for f in &files_for_verify {
            let path = Path::new(&f.temp_file_path);
            if !path.exists() {
                error!(
                    "File missing after analysis: {} (path: {})",
                    f.filename, f.temp_file_path
                );
            }
        }

        info!(
            "Analyzed job {} - detected: {:?} - {:?}",
            job_id, job.detected_artist, job.detected_album
        );

        Ok(())
    }

    /// Extract embedded tags from an audio file using ffprobe.
    async fn extract_tags(&self, path: &Path) -> Result<HashMap<String, String>> {
        use tokio::process::Command;

        let output = Command::new("ffprobe")
            .args(["-v", "quiet", "-print_format", "json", "-show_format"])
            .arg(path)
            .output()
            .await?;

        if !output.status.success() {
            anyhow::bail!("ffprobe failed");
        }

        let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        let mut tags = HashMap::new();

        if let Some(format_tags) = json.get("format").and_then(|f| f.get("tags")) {
            if let Some(obj) = format_tags.as_object() {
                for (key, value) in obj {
                    if let Some(v) = value.as_str() {
                        tags.insert(key.to_lowercase(), v.to_string());
                    }
                }
            }
        }

        Ok(tags)
    }

    /// Determine if a file needs conversion based on bitrate and format.
    fn determine_conversion_need(
        &self,
        bitrate: Option<i32>,
        _codec: &Option<String>,
        _temp_path: &Path,
    ) -> ConversionReason {
        let min_bitrate = self.config.target_bitrate as i32 - self.config.bitrate_tolerance as i32;
        let max_bitrate = self.config.target_bitrate as i32 + self.config.bitrate_tolerance as i32;

        let bitrate = match bitrate {
            Some(b) if b > 0 => b,
            _ => return ConversionReason::UndetectableBitrate,
        };

        if bitrate < min_bitrate {
            return ConversionReason::LowBitratePendingConfirmation {
                original_bitrate: bitrate,
            };
        }

        if bitrate > max_bitrate {
            return ConversionReason::HighBitrate {
                original_bitrate: bitrate,
            };
        }

        // Bitrate is within range - no conversion needed
        ConversionReason::NoConversionNeeded
    }

    /// Check if any files have low bitrate and create review if needed.
    async fn check_low_bitrate_files(
        &self,
        job_id: &str,
        job: &mut IngestionJob,
    ) -> Result<bool, IngestionError> {
        let files = self.store.get_files_for_job(job_id)?;
        let low_bitrate_files: Vec<_> = files
            .iter()
            .filter_map(|f| {
                if let Some(ConversionReason::LowBitratePendingConfirmation { original_bitrate }) =
                    &f.conversion_reason
                {
                    Some((f.filename.clone(), *original_bitrate))
                } else {
                    None
                }
            })
            .collect();

        if !low_bitrate_files.is_empty() {
            let files_list = low_bitrate_files
                .iter()
                .map(|(name, br)| format!("{} ({} kbps)", name, br))
                .collect::<Vec<_>>()
                .join("\n");

            let question = format!(
                "Audio quality is below target ({} kbps).\n\n\
                 The following files have low bitrate:\n{}\n\n\
                 Convert anyway or reject?",
                self.config.target_bitrate, files_list
            );

            let options = vec![
                ReviewOption {
                    id: "convert_low_bitrate".to_string(),
                    label: "Convert anyway".to_string(),
                    description: Some(format!(
                        "Convert low bitrate files to {}kbps OGG",
                        self.config.target_bitrate
                    )),
                },
                ReviewOption {
                    id: "no_match".to_string(),
                    label: "Reject upload".to_string(),
                    description: Some("Cancel this ingestion due to low quality".to_string()),
                },
            ];

            let options_json = serde_json::to_string(&options).unwrap_or_default();
            self.store
                .create_review_item(job_id, &question, &options_json)?;

            job.status = IngestionJobStatus::AwaitingReview;
            self.store.update_job(job)?;
            return Ok(true);
        }

        Ok(false)
    }

    /// Build aggregated metadata summary from all files in a job.
    fn build_metadata_summary(&self, job_id: &str) -> Result<AlbumMetadataSummary, IngestionError> {
        let job = self
            .store
            .get_job(job_id)?
            .ok_or_else(|| IngestionError::JobNotFound(job_id.to_string()))?;

        let files = self.store.get_files_for_job(job_id)?;

        // Count occurrences of each value to find most common
        let mut artist_counts: HashMap<String, usize> = HashMap::new();
        let mut album_counts: HashMap<String, usize> = HashMap::new();
        let mut years: Vec<i32> = Vec::new();
        let mut total_duration_ms: i64 = 0;
        let mut track_titles: Vec<(Option<i32>, String)> = Vec::new();

        for file in &files {
            if let Some(artist) = &file.tag_artist {
                *artist_counts.entry(artist.clone()).or_insert(0) += 1;
            }
            if let Some(album) = &file.tag_album {
                *album_counts.entry(album.clone()).or_insert(0) += 1;
            }
            if let Some(year) = file.tag_year {
                years.push(year);
            }
            if let Some(duration) = file.duration_ms {
                total_duration_ms += duration;
            }
            let title = file
                .tag_title
                .clone()
                .unwrap_or_else(|| file.filename.clone());
            track_titles.push((file.tag_track_num, title));
        }

        // Sort tracks by track number
        track_titles.sort_by_key(|(num, _)| num.unwrap_or(999));
        let track_titles: Vec<String> = track_titles.into_iter().map(|(_, t)| t).collect();

        // Get most common values
        let artist = artist_counts
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(name, _)| name);

        let album = album_counts
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(name, _)| name);

        // Use most common year
        let year = if !years.is_empty() {
            let mut year_counts: HashMap<i32, usize> = HashMap::new();
            for y in years {
                *year_counts.entry(y).or_insert(0) += 1;
            }
            year_counts
                .into_iter()
                .max_by_key(|(_, count)| *count)
                .map(|(y, _)| y)
        } else {
            None
        };

        Ok(AlbumMetadataSummary {
            artist,
            album,
            year,
            file_count: files.len() as i32,
            total_duration_ms,
            track_titles,
            source_name: job.original_filename,
        })
    }

    // =========================================================================
    // Phase 2: Identify Album (with LLM or heuristics)
    // =========================================================================

    /// Process a job in IDENTIFYING_ALBUM state - search catalog and find matching album.
    ///
    /// For download request jobs (context_type == DownloadRequest), the album is already
    /// known from the queue item, so we skip the search/scoring phase and directly verify
    /// the uploaded content matches the expected album.
    pub async fn process_job(&self, job_id: &str) -> Result<(), IngestionError> {
        let mut job = self
            .store
            .get_job(job_id)?
            .ok_or_else(|| IngestionError::JobNotFound(job_id.to_string()))?;

        // Handle different starting states
        match job.status {
            IngestionJobStatus::Pending => {
                // First analyze, then continue
                self.analyze_job(job_id).await?;
                job = self.store.get_job(job_id)?.unwrap();
            }
            IngestionJobStatus::IdentifyingAlbum => {
                // Continue with album identification
            }
            _ => {
                return Err(IngestionError::InvalidState {
                    expected: "PENDING or IDENTIFYING_ALBUM".to_string(),
                    actual: job.status.as_str().to_string(),
                });
            }
        }

        // Get metadata summary
        let summary = self.build_metadata_summary(job_id)?;

        // Check if this is a download request - if so, use the fast path
        if job.context_type == Some(IngestionContextType::DownloadRequest) {
            if let Some(context_id) = job.context_id.clone() {
                return self
                    .process_download_request_job(&mut job, &context_id, &summary)
                    .await;
            }
        }

        // Otherwise, use the normal search-based identification

        debug!(
            "Job {} metadata summary: artist={:?}, album={:?}, year={:?}, files={}, duration={}ms, tracks={:?}",
            job_id,
            summary.artist,
            summary.album,
            summary.year,
            summary.file_count,
            summary.total_duration_ms,
            summary.track_titles
        );

        // Try fingerprint matching first (works even without metadata tags)
        let ordered_durations = self.extract_ordered_durations(job_id).await?;
        if !ordered_durations.is_empty() {
            let fp_config = FingerprintConfig::default();
            let fp_result =
                match_album_with_fallbacks(&ordered_durations, self.catalog.as_ref(), &fp_config)?;

            match fp_result.ticket_type {
                TicketType::Success => {
                    let album = fp_result.matched_album.as_ref().unwrap();
                    job.matched_album_id = Some(album.id.clone());
                    job.match_confidence = Some(fp_result.match_score);
                    job.match_source = Some(IngestionMatchSource::Fingerprint);
                    job.status = IngestionJobStatus::MappingTracks;
                    self.store.update_job(&job)?;

                    info!(
                        "Fingerprint auto-matched job {} to album {} ({} - {}) with {:.0}% confidence, delta={}ms",
                        job_id, album.id, album.artist_name, album.name,
                        fp_result.match_score * 100.0, fp_result.total_delta_ms
                    );

                    if let Some(notifier) = &self.notifier {
                        use crate::server::websocket::messages::ingestion::CandidateSummary;
                        let candidates: Vec<CandidateSummary> = fp_result
                            .candidates
                            .iter()
                            .map(|c| CandidateSummary {
                                id: c.album.id.clone(),
                                name: c.album.name.clone(),
                                artist_name: c.album.artist_name.clone(),
                                track_count: c.album.track_count,
                                score: c.score,
                                delta_ms: c.delta_ms,
                            })
                            .collect();
                        notifier
                            .notify_match_found(&job, TicketType::Success, candidates)
                            .await;
                    }

                    self.map_tracks(job_id, false).await?;

                    let job_after_map = self.store.get_job(job_id)?.unwrap();
                    if job_after_map.tracks_matched == 0 {
                        let mut job = job_after_map;
                        self.fail_job_with_cleanup(
                            &mut job,
                            "No tracks could be matched — files may lack metadata tags or have corrupt audio data",
                        ).await?;

                        return Err(IngestionError::Store(anyhow::anyhow!(
                            "Zero tracks matched for job {}",
                            job_id
                        )));
                    }

                    self.convert_job(job_id).await?;
                    return Ok(());
                }
                TicketType::Review => {
                    let mut options: Vec<ReviewOption> = fp_result
                        .candidates
                        .iter()
                        .map(|c| ReviewOption {
                            id: format!("album:{}", c.album.id),
                            label: format!(
                                "{} - {} ({:.0}%, {} tracks, delta={}ms)",
                                c.album.artist_name,
                                c.album.name,
                                c.score * 100.0,
                                c.album.track_count,
                                c.delta_ms
                            ),
                            description: None,
                        })
                        .collect();
                    options.push(ReviewOption {
                        id: "no_match".to_string(),
                        label: "None of these".to_string(),
                        description: Some("Album not in catalog".to_string()),
                    });

                    let question = format!(
                        "Fingerprint matched candidates for '{}' ({} files).\nDetected: {} - {}",
                        job.original_filename,
                        summary.file_count,
                        summary.artist.as_deref().unwrap_or("Unknown Artist"),
                        summary.album.as_deref().unwrap_or("Unknown Album"),
                    );

                    let options_json = serde_json::to_string(&options).unwrap_or_default();
                    self.store
                        .create_review_item(job_id, &question, &options_json)?;

                    job.status = IngestionJobStatus::AwaitingReview;
                    self.store.update_job(&job)?;

                    if let Some(notifier) = &self.notifier {
                        notifier
                            .notify_review_needed(&job, &question, &options)
                            .await;
                    }

                    info!(
                        "Fingerprint review needed for job {} - best match: {:.0}%",
                        job_id,
                        fp_result.match_score * 100.0
                    );
                    return Ok(());
                }
                TicketType::Failure => {
                    debug!(
                        "Fingerprint matching failed for job {}, falling through to search-based identification",
                        job_id
                    );
                }
            }
        }

        // Search for matching albums in catalog
        let candidates = self.search_album_candidates(&summary).await?;

        debug!("Job {} found {} album candidates", job_id, candidates.len());

        if candidates.is_empty() {
            // No candidates found
            info!(
                "Job {} - no album candidates found for query: artist={:?}, album={:?}",
                job_id, summary.artist, summary.album
            );
            job.status = IngestionJobStatus::AwaitingReview;
            self.create_no_match_review(&job, &summary)?;
            self.store.update_job(&job)?;
            return Ok(());
        }

        // Score candidates (now includes track-based scoring)
        let mut scored: Vec<(AlbumCandidate, f32)> = candidates
            .into_iter()
            .map(|candidate| {
                let score = self.score_album_match(&summary, &candidate);
                debug!(
                    "Candidate {} - {} ({} tracks): score={:.2}",
                    candidate.artist_name, candidate.name, candidate.track_count, score
                );
                (candidate, score)
            })
            .collect();

        // Sort by score descending
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        if !scored.is_empty() {
            debug!(
                "Top candidate: {} - {} with score {:.2} (threshold: {:.2})",
                scored[0].0.artist_name,
                scored[0].0.name,
                scored[0].1,
                self.config.auto_match_threshold
            );
        }

        // Find best match
        if let Some((best_candidate, confidence)) = scored.first() {
            let label = format!("{} - {}", best_candidate.artist_name, best_candidate.name);

            if *confidence >= self.config.auto_match_threshold {
                // High confidence - auto-match
                job.matched_album_id = Some(best_candidate.id.clone());
                job.match_confidence = Some(*confidence);
                job.match_source = Some(IngestionMatchSource::Agent);
                job.status = IngestionJobStatus::MappingTracks;
                self.store.update_job(&job)?;

                info!(
                    "Auto-matched job {} to album {} with {:.0}% confidence (tracks: {}/{})",
                    job_id,
                    best_candidate.id,
                    confidence * 100.0,
                    summary.file_count,
                    best_candidate.track_count
                );

                // Notify match found
                if let Some(notifier) = &self.notifier {
                    use crate::server::websocket::messages::ingestion::CandidateSummary;
                    let candidates: Vec<CandidateSummary> = scored
                        .iter()
                        .take(5)
                        .map(|(c, s)| CandidateSummary {
                            id: c.id.clone(),
                            name: c.name.clone(),
                            artist_name: c.artist_name.clone(),
                            track_count: c.track_count,
                            score: *s,
                            delta_ms: 0, // Not available from this scoring path
                        })
                        .collect();
                    notifier
                        .notify_match_found(&job, TicketType::Success, candidates)
                        .await;
                }

                // Continue to track mapping and conversion
                self.map_tracks(job_id, false).await?;

                // Fail if no tracks could be matched
                let job_after_map = self.store.get_job(job_id)?.unwrap();
                if job_after_map.tracks_matched == 0 {
                    let mut job = job_after_map;
                    self.fail_job_with_cleanup(
                        &mut job,
                        "No tracks could be matched — files may lack metadata tags or have corrupt audio data",
                    ).await?;

                    return Err(IngestionError::Store(anyhow::anyhow!(
                        "Zero tracks matched for job {}",
                        job_id
                    )));
                }

                self.convert_job(job_id).await?;
            } else {
                // Low confidence - request review
                let options: Vec<ReviewOption> = scored
                    .iter()
                    .take(5)
                    .map(|(candidate, conf)| ReviewOption {
                        id: format!("album:{}", candidate.id),
                        label: format!(
                            "{} - {} ({:.0}%, {} tracks)",
                            candidate.artist_name,
                            candidate.name,
                            conf * 100.0,
                            candidate.track_count
                        ),
                        description: None,
                    })
                    .chain(std::iter::once(ReviewOption {
                        id: "no_match".to_string(),
                        label: "None of these".to_string(),
                        description: Some("Album not in catalog".to_string()),
                    }))
                    .collect();

                let question = format!(
                    "Which album is this?\nDetected: {} - {} ({} files)",
                    summary.artist.as_deref().unwrap_or("Unknown Artist"),
                    summary.album.as_deref().unwrap_or("Unknown Album"),
                    summary.file_count
                );

                let options_json = serde_json::to_string(&options).unwrap_or_default();
                self.store
                    .create_review_item(job_id, &question, &options_json)?;

                job.status = IngestionJobStatus::AwaitingReview;
                self.store.update_job(&job)?;

                // Notify review needed
                if let Some(notifier) = &self.notifier {
                    notifier
                        .notify_review_needed(&job, &question, &options)
                        .await;
                }

                info!(
                    "Job {} requires review - best match: {} ({:.0}%)",
                    job_id,
                    label,
                    confidence * 100.0
                );
            }
        }

        Ok(())
    }

    /// Process a download request job - album is already known, just verify and proceed.
    ///
    /// For download requests, we skip the search/scoring phase because the album ID
    /// is already specified in the queue item. We still verify the uploaded content
    /// is a reasonable match (track count, duration) before proceeding.
    async fn process_download_request_job(
        &self,
        job: &mut IngestionJob,
        queue_item_id: &str,
        summary: &AlbumMetadataSummary,
    ) -> Result<(), IngestionError> {
        let job_id = job.id.clone();

        // Get the queue item to find the album ID
        let queue_item = match &self.download_manager {
            Some(dm) => dm.get_queue_item(queue_item_id).map_err(|e| {
                IngestionError::Store(anyhow::anyhow!(
                    "Failed to get queue item {}: {}",
                    queue_item_id,
                    e
                ))
            })?,
            None => {
                warn!(
                    "Job {} has download request context but no download manager configured",
                    job_id
                );
                return Err(IngestionError::Store(anyhow::anyhow!(
                    "Download manager not configured"
                )));
            }
        };

        let queue_item = match queue_item {
            Some(item) => item,
            None => {
                warn!(
                    "Job {} references non-existent queue item {}",
                    job_id, queue_item_id
                );
                return Err(IngestionError::Store(anyhow::anyhow!(
                    "Queue item {} not found",
                    queue_item_id
                )));
            }
        };

        let album_id = &queue_item.content_id;
        let album_name = queue_item
            .content_name
            .as_deref()
            .unwrap_or("Unknown Album");
        let artist_name = queue_item
            .artist_name
            .as_deref()
            .unwrap_or("Unknown Artist");

        info!(
            "Job {} is a download request for album {} ({} - {})",
            job_id, album_id, artist_name, album_name
        );

        // Verify using duration fingerprint comparison
        let uploaded_durations = self.extract_ordered_durations(&job_id).await?;
        let catalog_durations = self
            .catalog
            .get_album_track_durations(album_id)
            .map_err(|e| {
                IngestionError::Store(anyhow::anyhow!(
                    "Failed to get track durations for album {}: {}",
                    album_id,
                    e
                ))
            })?;

        // Also compute metadata score as a secondary signal
        let metadata_score = if let Some(candidate) = self.build_album_candidate(album_id) {
            self.score_album_match(summary, &candidate)
        } else {
            0.0
        };

        let fp_config = FingerprintConfig::default();
        let (fp_matches, fp_delta) =
            if !uploaded_durations.is_empty() && !catalog_durations.is_empty() {
                compare_durations(
                    &uploaded_durations,
                    &catalog_durations,
                    fp_config.track_tolerance_ms,
                )
            } else {
                (0, 0)
            };
        let total_tracks = uploaded_durations.len().max(catalog_durations.len());
        let fp_score = if total_tracks > 0 {
            fp_matches as f32 / total_tracks as f32
        } else {
            0.0
        };

        debug!(
            "Job {} download request verification: album={}, fp_score={:.2} ({}/{} tracks, delta={}ms), metadata_score={:.2}",
            job_id, album_id, fp_score, fp_matches, total_tracks, fp_delta, metadata_score
        );

        if fp_score >= 1.0 && fp_delta < fp_config.auto_ingest_delta_threshold_ms {
            // Perfect fingerprint match — auto-proceed
        } else if fp_score >= 0.9 {
            // High but not perfect — review with details
            let options = vec![
                ReviewOption {
                    id: format!("album:{}", album_id),
                    label: format!(
                        "{} - {} (fingerprint {:.0}%, metadata {:.0}%)",
                        artist_name,
                        album_name,
                        fp_score * 100.0,
                        metadata_score * 100.0
                    ),
                    description: Some("Proceed with this album".to_string()),
                },
                ReviewOption {
                    id: "no_match".to_string(),
                    label: "Content doesn't match".to_string(),
                    description: Some("Reject this upload".to_string()),
                },
            ];

            let question = format!(
                "Downloaded content for '{}' has near-match fingerprint ({:.0}%, delta={}ms).\n\
                 Metadata score: {:.0}%\n\
                 Expected: {} tracks, Uploaded: {} files\n\
                 Confirm this is the correct album:",
                album_name,
                fp_score * 100.0,
                fp_delta,
                metadata_score * 100.0,
                catalog_durations.len(),
                summary.file_count
            );

            let options_json = serde_json::to_string(&options).unwrap_or_default();
            self.store
                .create_review_item(&job_id, &question, &options_json)?;

            job.matched_album_id = Some(album_id.clone());
            job.match_confidence = Some(fp_score);
            job.match_source = Some(IngestionMatchSource::DownloadRequest);
            job.status = IngestionJobStatus::AwaitingReview;
            self.store.update_job(job)?;

            if let Some(notifier) = &self.notifier {
                notifier
                    .notify_review_needed(job, &question, &options)
                    .await;
            }

            return Ok(());
        } else {
            // Low fingerprint score — review with warning
            warn!(
                "Job {} - download request fingerprint score low ({:.2}), requesting review",
                job_id, fp_score
            );

            let options = vec![
                ReviewOption {
                    id: format!("album:{}", album_id),
                    label: format!(
                        "{} - {} (fingerprint {:.0}%, metadata {:.0}%)",
                        artist_name,
                        album_name,
                        fp_score * 100.0,
                        metadata_score * 100.0
                    ),
                    description: Some("Proceed with this album anyway".to_string()),
                },
                ReviewOption {
                    id: "no_match".to_string(),
                    label: "Content doesn't match".to_string(),
                    description: Some("Reject this upload".to_string()),
                },
            ];

            let question = format!(
                "Downloaded content for '{}' has low fingerprint match ({:.0}%, delta={}ms).\n\
                 Metadata score: {:.0}%\n\
                 Expected: {} tracks, Uploaded: {} files\n\
                 Confirm this is the correct album:",
                album_name,
                fp_score * 100.0,
                fp_delta,
                metadata_score * 100.0,
                catalog_durations.len(),
                summary.file_count
            );

            let options_json = serde_json::to_string(&options).unwrap_or_default();
            self.store
                .create_review_item(&job_id, &question, &options_json)?;

            job.matched_album_id = Some(album_id.clone());
            job.match_confidence = Some(fp_score);
            job.match_source = Some(IngestionMatchSource::DownloadRequest);
            job.status = IngestionJobStatus::AwaitingReview;
            self.store.update_job(job)?;

            if let Some(notifier) = &self.notifier {
                notifier
                    .notify_review_needed(job, &question, &options)
                    .await;
            }

            return Ok(());
        }

        // Perfect fingerprint match - proceed directly to track mapping
        job.matched_album_id = Some(album_id.clone());
        job.match_confidence = Some(fp_score);
        job.match_source = Some(IngestionMatchSource::DownloadRequest);
        job.status = IngestionJobStatus::MappingTracks;
        self.store.update_job(job)?;

        info!(
            "Download request job {} matched to album {} with fingerprint {:.0}% (delta={}ms, metadata {:.0}%, tracks: {}/{})",
            job_id, album_id,
            fp_score * 100.0, fp_delta,
            metadata_score * 100.0,
            summary.file_count,
            catalog_durations.len()
        );

        // Continue to track mapping and conversion
        self.map_tracks(&job_id, false).await?;

        // Fail if no tracks could be matched
        let job_after_map = self.store.get_job(&job_id)?.unwrap();
        if job_after_map.tracks_matched == 0 {
            let mut job = job_after_map;
            self.fail_job_with_cleanup(
                &mut job,
                "No tracks could be matched — files may lack metadata tags or have corrupt audio data",
            ).await?;

            return Err(IngestionError::Store(anyhow::anyhow!(
                "Zero tracks matched for job {}",
                job_id
            )));
        }

        self.convert_job(&job_id).await?;

        Ok(())
    }

    /// Search catalog for album candidates matching the summary.
    async fn search_album_candidates(
        &self,
        summary: &AlbumMetadataSummary,
    ) -> Result<Vec<AlbumCandidate>, IngestionError> {
        let mut candidates = Vec::new();

        // Collect album IDs first, then fetch full details
        let mut album_ids: Vec<String> = Vec::new();
        let album_filter = Some(vec![HashedItemType::Album]);

        // Strategy 1: Search for album by name only
        if let Some(album_name) = &summary.album {
            debug!("Searching albums by name: {:?}", album_name);
            let results = self.search.search(album_name, 10, album_filter.clone());
            debug!("Album name search returned {} results", results.len());
            for result in results {
                album_ids.push(result.item_id.clone());
            }
        }

        // Strategy 2: Search for artist and include their albums
        if let Some(artist_name) = &summary.artist {
            debug!("Searching artists by name: {:?}", artist_name);
            let artist_filter = Some(vec![HashedItemType::Artist]);
            let artist_results = self.search.search(artist_name, 5, artist_filter);
            debug!("Artist search returned {} results", artist_results.len());

            for result in artist_results {
                if let Ok(Some(artist_json)) =
                    self.catalog.get_resolved_artist_json(&result.item_id)
                {
                    if let Some(albums) = artist_json.get("albums").and_then(|a| a.as_array()) {
                        // Include all albums from matched artists for scoring
                        for album in albums.iter().take(20) {
                            if let Some(album_id) = album.get("id").and_then(|v| v.as_str()) {
                                if !album_id.is_empty() {
                                    album_ids.push(album_id.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }

        // Strategy 3: Fallback to source filename if no artist/album detected
        if album_ids.is_empty() {
            debug!(
                "Falling back to source filename search: {:?}",
                summary.source_name
            );
            let results = self.search.search(&summary.source_name, 10, album_filter);
            debug!("Filename search returned {} results", results.len());
            for result in results {
                album_ids.push(result.item_id.clone());
            }
        }

        // Deduplicate IDs
        album_ids.sort();
        album_ids.dedup();

        debug!(
            "Total unique album IDs to evaluate: {} - {:?}",
            album_ids.len(),
            album_ids
        );

        // Fetch full album details with tracks for each candidate
        for album_id in &album_ids {
            if let Some(candidate) = self.build_album_candidate(album_id) {
                candidates.push(candidate);
            }
        }

        Ok(candidates)
    }

    /// Build an AlbumCandidate from a resolved album JSON.
    fn build_album_candidate(&self, album_id: &str) -> Option<AlbumCandidate> {
        let album_json = self.catalog.get_resolved_album_json(album_id).ok()??;

        let album = album_json.get("album")?;
        let artists = album_json.get("artists")?.as_array()?;

        let id = album.get("id")?.as_str()?.to_string();
        let name = album.get("name")?.as_str()?.to_string();
        let artist_name = artists
            .first()
            .and_then(|a| a.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown Artist")
            .to_string();

        // Extract track info from discs
        let mut track_titles = Vec::new();
        let mut total_duration_ms: i64 = 0;
        let mut track_count = 0;

        if let Some(discs) = album_json.get("discs").and_then(|d| d.as_array()) {
            for disc in discs {
                if let Some(tracks) = disc.get("tracks").and_then(|t| t.as_array()) {
                    for track in tracks {
                        track_count += 1;
                        if let Some(title) = track.get("name").and_then(|v| v.as_str()) {
                            track_titles.push(title.to_string());
                        }
                        if let Some(duration) = track.get("duration_ms").and_then(|v| v.as_i64()) {
                            total_duration_ms += duration;
                        }
                    }
                }
            }
        }

        Some(AlbumCandidate {
            id,
            name,
            artist_name,
            track_count,
            total_duration_ms,
            track_titles,
        })
    }

    /// Score how well an album candidate matches the detected metadata.
    ///
    /// Scoring weights:
    /// - 25% Artist name similarity
    /// - 25% Album name similarity
    /// - 15% Track count match
    /// - 15% Track title overlap
    /// - 10% Total duration similarity
    /// - 10% Source filename similarity
    fn score_album_match(&self, summary: &AlbumMetadataSummary, candidate: &AlbumCandidate) -> f32 {
        let mut score = 0.0;
        let mut factors = 0.0;

        // Artist similarity (25%)
        if let Some(detected_artist) = &summary.artist {
            let sim = string_similarity(detected_artist, &candidate.artist_name);
            score += sim * 0.25;
            factors += 0.25;
        }

        // Album name similarity (25%)
        if let Some(detected_album) = &summary.album {
            let sim = string_similarity(detected_album, &candidate.name);
            score += sim * 0.25;
            factors += 0.25;
        }

        // Track count match (15%)
        // Perfect match = 1.0, each track difference reduces by 0.1
        let track_diff = (summary.file_count - candidate.track_count).abs();
        let track_count_score = (1.0 - (track_diff as f32 * 0.1)).max(0.0);
        score += track_count_score * 0.15;
        factors += 0.15;

        // Track title overlap (15%)
        // Calculate how many uploaded track titles match catalog track titles
        if !summary.track_titles.is_empty() && !candidate.track_titles.is_empty() {
            let mut matched_titles = 0;
            for upload_title in &summary.track_titles {
                // Find best match in candidate tracks
                let best_sim = candidate
                    .track_titles
                    .iter()
                    .map(|t| string_similarity(upload_title, t))
                    .max_by(|a, b| a.partial_cmp(b).unwrap())
                    .unwrap_or(0.0);
                if best_sim > 0.7 {
                    matched_titles += 1;
                }
            }
            let title_overlap = matched_titles as f32 / summary.track_titles.len() as f32;
            score += title_overlap * 0.15;
            factors += 0.15;
        }

        // Duration similarity (10%)
        // Allow 10% tolerance, perfect match = 1.0
        if summary.total_duration_ms > 0 && candidate.total_duration_ms > 0 {
            let duration_ratio =
                summary.total_duration_ms as f64 / candidate.total_duration_ms as f64;
            let duration_diff = (1.0 - duration_ratio).abs();
            let duration_score = (1.0 - duration_diff * 5.0).max(0.0) as f32; // 20% diff = 0 score
            score += duration_score * 0.10;
            factors += 0.10;
        }

        // Source filename similarity (10%)
        let source_sim = string_similarity(
            &summary.source_name,
            &format!("{} - {}", candidate.artist_name, candidate.name),
        );
        score += source_sim * 0.10;
        factors += 0.10;

        if factors > 0.0 {
            score / factors
        } else {
            0.0
        }
    }

    fn create_no_match_review(
        &self,
        job: &IngestionJob,
        summary: &AlbumMetadataSummary,
    ) -> Result<(), IngestionError> {
        let question = format!(
            "Could not find album in catalog.\nDetected: {} - {}\nFilename: {}",
            summary.artist.as_deref().unwrap_or("Unknown Artist"),
            summary.album.as_deref().unwrap_or("Unknown Album"),
            job.original_filename
        );

        let options = vec![
            ReviewOption {
                id: "retry".to_string(),
                label: "Search again".to_string(),
                description: None,
            },
            ReviewOption {
                id: "no_match".to_string(),
                label: "Album not in catalog".to_string(),
                description: Some("Mark as failed".to_string()),
            },
        ];

        let options_json = serde_json::to_string(&options).unwrap_or_default();
        self.store
            .create_review_item(&job.id, &question, &options_json)?;

        Ok(())
    }

    // =========================================================================
    // Phase 3: Map Tracks
    // =========================================================================

    /// Map files to tracks within the matched album.
    ///
    /// If `skip_duration_review` is true, duration mismatches will not create a review
    /// (used when called from resolve_review to avoid infinite loops).
    pub async fn map_tracks(
        &self,
        job_id: &str,
        skip_duration_review: bool,
    ) -> Result<(), IngestionError> {
        let mut job = self
            .store
            .get_job(job_id)?
            .ok_or_else(|| IngestionError::JobNotFound(job_id.to_string()))?;

        if job.status != IngestionJobStatus::MappingTracks {
            return Err(IngestionError::InvalidState {
                expected: "MAPPING_TRACKS".to_string(),
                actual: job.status.as_str().to_string(),
            });
        }

        let album_id = job
            .matched_album_id
            .as_ref()
            .ok_or(IngestionError::AlbumNotMatched)?;

        // Get album with tracks via resolved JSON
        let album_json = self
            .catalog
            .get_resolved_album_json(album_id)?
            .ok_or_else(|| {
                IngestionError::Store(anyhow::anyhow!("Album not found: {}", album_id))
            })?;

        // Parse tracks from discs array
        struct TrackInfo {
            id: String,
            name: String,
            track_number: i32,
            disc_number: i32,
            duration_ms: i64,
        }

        let mut tracks: Vec<TrackInfo> = Vec::new();
        if let Some(discs) = album_json.get("discs").and_then(|d| d.as_array()) {
            for disc in discs {
                let disc_number = disc.get("number").and_then(|n| n.as_i64()).unwrap_or(1) as i32;
                if let Some(disc_tracks) = disc.get("tracks").and_then(|t| t.as_array()) {
                    for track in disc_tracks {
                        let id = track.get("id").and_then(|v| v.as_str()).unwrap_or_default();
                        let name = track
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default();
                        let track_number = track
                            .get("track_number")
                            .and_then(|n| n.as_i64())
                            .unwrap_or(0) as i32;
                        let duration_ms = track
                            .get("duration_ms")
                            .and_then(|n| n.as_i64())
                            .unwrap_or(0);

                        if !id.is_empty() {
                            tracks.push(TrackInfo {
                                id: id.to_string(),
                                name: name.to_string(),
                                track_number,
                                disc_number,
                                duration_ms,
                            });
                        }
                    }
                }
            }
        }

        let mut files = self.store.get_files_for_job(job_id)?;

        // Verify files exist at start of mapping
        for f in &files {
            let path = Path::new(&f.temp_file_path);
            if !path.exists() {
                error!(
                    "File missing at map_tracks start: {} (path: {})",
                    f.filename, f.temp_file_path
                );
            }
        }

        info!(
            "Mapping {} files to {} tracks in album {}",
            files.len(),
            tracks.len(),
            album_id
        );

        // Build track lookup by (disc_number, track_number)
        let tracks_by_num: HashMap<(i32, i32), &TrackInfo> = tracks
            .iter()
            .map(|t| ((t.disc_number, t.track_number), t))
            .collect();

        let mut matched = 0;
        let mut claimed_track_ids: std::collections::HashSet<String> =
            std::collections::HashSet::new();

        for file in &mut files {
            // Try to match by disc + track number first
            let disc_num = file.tag_disc_num.unwrap_or(1);
            if let Some(track_num) = file.tag_track_num {
                if let Some(track) = tracks_by_num.get(&(disc_num, track_num)) {
                    file.matched_track_id = Some(track.id.clone());
                    file.match_confidence = Some(1.0);
                    claimed_track_ids.insert(track.id.clone());
                    matched += 1;
                    self.store.update_file(file)?;
                    continue;
                }
            }

            // Fall back to title matching
            if let Some(title) = &file.tag_title {
                let best_match = tracks
                    .iter()
                    .filter(|t| !claimed_track_ids.contains(&t.id))
                    .map(|t| (t, string_similarity(title, &t.name)))
                    .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

                if let Some((track, confidence)) = best_match {
                    if confidence > 0.7 {
                        file.matched_track_id = Some(track.id.clone());
                        file.match_confidence = Some(confidence);
                        claimed_track_ids.insert(track.id.clone());
                        matched += 1;
                        self.store.update_file(file)?;
                    }
                }
            }
        }

        // Duration-based fallback: match remaining files by closest track duration.
        // This handles the case where files have no embedded tags but durations
        // are unique enough to identify tracks (common after fingerprint matching).
        let unmatched_count = files
            .iter()
            .filter(|f| f.matched_track_id.is_none())
            .count();
        if unmatched_count > 0 {
            debug!(
                "Tag-based matching left {} unmatched files, trying duration-based mapping",
                unmatched_count
            );

            // Build scored pairs: (file_index, track_index, duration_delta, name_similarity)
            let mut pairs: Vec<(usize, usize, i64, f32)> = Vec::new();
            for (fi, file) in files.iter().enumerate() {
                if file.matched_track_id.is_some() {
                    continue;
                }
                let file_duration = match file.duration_ms {
                    Some(d) => d,
                    None => continue,
                };
                let file_stem = Path::new(&file.filename)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(&file.filename);
                for (ti, track) in tracks.iter().enumerate() {
                    if claimed_track_ids.contains(&track.id) {
                        continue;
                    }
                    let delta = (file_duration - track.duration_ms).abs();
                    let name_sim = string_similarity(file_stem, &track.name);
                    pairs.push((fi, ti, delta, name_sim));
                }
            }

            // Sort by duration delta ascending, then name similarity descending as tiebreaker
            pairs.sort_by(|a, b| a.2.cmp(&b.2).then(b.3.partial_cmp(&a.3).unwrap()));

            let mut claimed_files: std::collections::HashSet<usize> =
                std::collections::HashSet::new();
            for (fi, ti, delta, name_sim) in pairs {
                if claimed_files.contains(&fi) || claimed_track_ids.contains(&tracks[ti].id) {
                    continue;
                }
                // Confidence based on duration proximity (10s threshold → 1.0, worse → lower)
                let duration_confidence = (1.0 - (delta as f64 / 10_000.0).min(1.0)) as f32;
                // Blend: 70% duration, 30% name similarity
                let confidence = duration_confidence * 0.7 + name_sim * 0.3;
                if confidence > 0.3 {
                    files[fi].matched_track_id = Some(tracks[ti].id.clone());
                    files[fi].match_confidence = Some(confidence);
                    claimed_track_ids.insert(tracks[ti].id.clone());
                    claimed_files.insert(fi);
                    matched += 1;
                    self.store.update_file(&files[fi])?;
                    debug!(
                        "Duration-matched '{}' → '{}' (delta={}ms, name_sim={:.2}, conf={:.2})",
                        files[fi].filename, tracks[ti].name, delta, name_sim, confidence
                    );
                }
            }
        }

        job.tracks_matched = matched;

        info!(
            "Mapped {}/{} files for job {}",
            matched,
            files.len(),
            job_id
        );

        // Validate durations - flag for review if any track differs by > 10 seconds
        const DURATION_THRESHOLD_MS: i64 = 10_000;
        let tracks_by_id: HashMap<&str, &TrackInfo> =
            tracks.iter().map(|t| (t.id.as_str(), t)).collect();

        // Re-fetch files to get the updated matched_track_id values
        let files = self.store.get_files_for_job(job_id)?;
        let mut duration_mismatches: Vec<String> = Vec::new();

        for file in &files {
            if let (Some(track_id), Some(file_duration)) =
                (&file.matched_track_id, file.duration_ms)
            {
                if let Some(track) = tracks_by_id.get(track_id.as_str()) {
                    let delta = (file_duration - track.duration_ms).abs();
                    if delta > DURATION_THRESHOLD_MS {
                        debug!(
                            "Duration mismatch for {}: file={}ms, catalog={}ms, delta={}ms",
                            file.filename, file_duration, track.duration_ms, delta
                        );
                        duration_mismatches.push(format!(
                            "{}: {}s vs {}s (delta: {}s)",
                            track.name,
                            file_duration / 1000,
                            track.duration_ms / 1000,
                            delta / 1000
                        ));
                    }
                }
            }
        }

        if !duration_mismatches.is_empty() && !skip_duration_review {
            // Flag for review due to duration mismatches (unless skipped)
            let question = format!(
                "Duration mismatch detected for {} track(s):\n{}",
                duration_mismatches.len(),
                duration_mismatches.join("\n")
            );

            let options = vec![
                ReviewOption {
                    id: "continue".to_string(),
                    label: "Continue anyway".to_string(),
                    description: Some("Accept the files despite duration differences".to_string()),
                },
                ReviewOption {
                    id: "no_match".to_string(),
                    label: "Reject".to_string(),
                    description: Some("These files don't match the album".to_string()),
                },
            ];

            let options_json = serde_json::to_string(&options).unwrap_or_default();
            self.store
                .create_review_item(job_id, &question, &options_json)?;

            job.status = IngestionJobStatus::AwaitingReview;
            self.store.update_job(&job)?;

            // Notify review needed via WebSocket
            if let Some(notifier) = &self.notifier {
                notifier
                    .notify_review_needed(&job, &question, &options)
                    .await;
            }

            warn!(
                "Job {} flagged for review: {} duration mismatches",
                job_id,
                duration_mismatches.len()
            );

            return Ok(());
        }

        job.status = IngestionJobStatus::Converting;
        self.store.update_job(&job)?;

        Ok(())
    }

    // =========================================================================
    // Phase 4: Convert Files
    // =========================================================================

    /// Convert all matched files to OGG Vorbis.
    pub async fn convert_job(&self, job_id: &str) -> Result<(), IngestionError> {
        let mut job = self
            .store
            .get_job(job_id)?
            .ok_or_else(|| IngestionError::JobNotFound(job_id.to_string()))?;

        if job.status != IngestionJobStatus::Converting {
            return Err(IngestionError::InvalidState {
                expected: "CONVERTING".to_string(),
                actual: job.status.as_str().to_string(),
            });
        }

        let files = self.store.get_files_for_job(job_id)?;
        let mut converted = 0;
        let mut converted_track_ids: Vec<String> = Vec::new();

        for mut file in files {
            // Skip files without a matched track
            let track_id = match &file.matched_track_id {
                Some(id) => id.clone(),
                None => {
                    debug!("Skipping file {} - no matched track", file.filename);
                    continue;
                }
            };

            let input_path = Path::new(&file.temp_file_path);

            // Check if conversion is needed
            let needs_conversion = matches!(
                file.conversion_reason,
                Some(ConversionReason::HighBitrate { .. })
                    | Some(ConversionReason::LowBitrateApproved { .. })
                    | Some(ConversionReason::UndetectableBitrate)
            );

            if !needs_conversion {
                // No conversion needed - copy file directly to output
                let output_path = self
                    .file_handler
                    .get_output_path(&self.config.media_dir, &track_id);

                // Determine output extension based on original format
                let extension = Path::new(&file.filename)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("ogg");

                let output_path_with_ext = output_path.with_extension(extension);

                // Ensure output directory exists
                if let Some(parent) = output_path_with_ext.parent() {
                    if let Err(e) = tokio::fs::create_dir_all(parent).await {
                        error!("Failed to create output directory {:?}: {}", parent, e);
                    }
                }

                // Check if input file exists before attempting copy
                let input_exists = input_path.exists();
                if !input_exists {
                    error!(
                        "Input file does not exist: {:?} (temp_file_path: {})",
                        input_path, file.temp_file_path
                    );
                }

                match tokio::fs::copy(&input_path, &output_path_with_ext).await {
                    Ok(_) => {
                        file.output_file_path =
                            Some(output_path_with_ext.to_string_lossy().to_string());
                        file.converted = true; // Mark as processed even if not transcoded
                        converted += 1;
                        converted_track_ids.push(track_id.clone());

                        // Update catalog with appropriate extension (sharded path)
                        let (dir1, dir2) = super::file_handler::FileHandler::shard_dirs(&track_id);
                        let audio_uri =
                            format!("audio/{}/{}/{}.{}", dir1, dir2, track_id, extension);
                        if let Err(e) = self.catalog.set_track_audio_uri(&track_id, &audio_uri) {
                            warn!("Failed to update track {} audio_uri: {}", track_id, e);
                        }

                        info!(
                            "Copied {} -> {} (no conversion needed, {} kbps)",
                            file.filename,
                            track_id,
                            file.bitrate.unwrap_or(0)
                        );
                    }
                    Err(e) => {
                        error!(
                            "Failed to copy {} from {:?} to {:?}: {}",
                            file.filename, input_path, output_path_with_ext, e
                        );
                        file.error_message = Some(e.to_string());
                    }
                }

                self.store.update_file(&file)?;
                continue;
            }

            // Original conversion logic for files that need conversion
            let output_path = self
                .file_handler
                .get_output_path(&self.config.media_dir, &track_id);

            match convert_to_ogg(input_path, &output_path, self.config.target_bitrate).await {
                Ok(()) => {
                    file.output_file_path = Some(output_path.to_string_lossy().to_string());
                    file.converted = true;
                    converted += 1;
                    converted_track_ids.push(track_id.clone());

                    // Update catalog: set audio_uri for the track (sharded path)
                    let (dir1, dir2) = super::file_handler::FileHandler::shard_dirs(&track_id);
                    let audio_uri = format!("audio/{}/{}/{}.ogg", dir1, dir2, track_id);
                    if let Err(e) = self.catalog.set_track_audio_uri(&track_id, &audio_uri) {
                        warn!("Failed to update track {} audio_uri: {}", track_id, e);
                    }

                    info!(
                        "Converted {} -> {} (target: {} kbps)",
                        file.filename, track_id, self.config.target_bitrate
                    );
                }
                Err(e) => {
                    error!("Failed to convert {}: {}", file.filename, e);
                    file.error_message = Some(e.to_string());
                }
            }

            self.store.update_file(&file)?;
        }

        // Update album availability in catalog
        if let Some(album_id) = &job.matched_album_id {
            match self.catalog.recompute_album_availability(album_id) {
                Ok(availability) => {
                    info!(
                        "Album {} availability updated to {:?}",
                        album_id, availability
                    );

                    // Update search index for album
                    let album_available =
                        availability != crate::catalog_store::AlbumAvailability::Missing;
                    self.search.update_availability(&[(
                        album_id.clone(),
                        HashedItemType::Album,
                        album_available,
                    )]);

                    // Update artist availability for album's artists
                    match self.catalog.get_album_artist_ids(album_id) {
                        Ok(artist_ids) => {
                            for artist_id in artist_ids {
                                match self.catalog.recompute_artist_availability(&artist_id) {
                                    Ok(artist_available) => {
                                        info!(
                                            "Artist {} availability updated to {}",
                                            artist_id, artist_available
                                        );
                                        self.search.update_availability(&[(
                                            artist_id,
                                            HashedItemType::Artist,
                                            artist_available,
                                        )]);
                                    }
                                    Err(e) => {
                                        warn!(
                                            "Failed to recompute artist {} availability: {}",
                                            artist_id, e
                                        );
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Failed to get artist IDs for album {}: {}", album_id, e);
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to recompute album {} availability: {}", album_id, e);
                }
            }
        }

        // Update search index for converted tracks
        let track_availability_updates: Vec<_> = converted_track_ids
            .iter()
            .map(|id| (id.clone(), HashedItemType::Track, true))
            .collect();
        if !track_availability_updates.is_empty() {
            self.search.update_availability(&track_availability_updates);
        }

        job.tracks_converted = converted;
        job.status = IngestionJobStatus::Completed;
        job.completed_at = Some(chrono::Utc::now().timestamp_millis());
        self.store.update_job(&job)?;

        // If this job is associated with a download request, mark it as completed
        let mut primary_requester_user_id: Option<String> = None;
        let mut primary_requester_request_id: Option<String> = None;
        let mut download_request_album_name: Option<String> = None;
        let mut download_request_artist_name: Option<String> = None;
        if let (Some(IngestionContextType::DownloadRequest), Some(context_id)) =
            (job.context_type, &job.context_id)
        {
            if let Some(download_manager) = &self.download_manager {
                // Capture the requesting user and names before marking completed
                if let Ok(Some(queue_item)) = download_manager.get_queue_item(context_id) {
                    primary_requester_user_id = queue_item.requested_by_user_id.clone();
                    primary_requester_request_id = Some(queue_item.id.clone());
                    download_request_album_name = queue_item.content_name.clone();
                    download_request_artist_name = queue_item.artist_name.clone();
                }

                let duration_ms = job.started_at.map_or(0, |started| {
                    job.completed_at
                        .unwrap_or(chrono::Utc::now().timestamp_millis())
                        - started
                });

                if let Err(e) = download_manager.mark_request_completed(
                    context_id,
                    job.total_size_bytes as u64,
                    duration_ms,
                ) {
                    error!(
                        "Failed to mark download request {} as completed: {}",
                        context_id, e
                    );
                } else {
                    info!(
                        "Marked download request {} as completed for job {}",
                        context_id, job_id
                    );
                }
            }
        }

        // Auto-complete any other pending download requests for the same album
        let mut auto_completed_requests: Vec<CompletedRequestInfo> = Vec::new();
        if let Some(album_id) = &job.matched_album_id {
            if let Some(download_manager) = &self.download_manager {
                let duration_ms = job.started_at.map_or(0, |started| {
                    job.completed_at
                        .unwrap_or(chrono::Utc::now().timestamp_millis())
                        - started
                });

                match download_manager.complete_requests_for_album(
                    album_id,
                    job.total_size_bytes as u64,
                    duration_ms,
                ) {
                    Ok(completed) if !completed.is_empty() => {
                        info!(
                            "Auto-completed {} additional download request(s) for album {}: {:?}",
                            completed.len(),
                            album_id,
                            completed
                        );
                        auto_completed_requests = completed;
                    }
                    Ok(_) => {} // No additional requests to complete
                    Err(e) => {
                        warn!(
                            "Failed to auto-complete download requests for album {}: {}",
                            album_id, e
                        );
                    }
                }
            }
        }

        // Cleanup temp files
        let _ = self.file_handler.cleanup_job(job_id).await;

        // Notify completion — look up names from catalog for accuracy,
        // falling back to download request names, detected metadata, or "Unknown" defaults.
        let (album_name, artist_name) = match &job.matched_album_id {
            Some(album_id) => {
                let catalog_album_name = match self.catalog.get_album_json(album_id) {
                    Ok(Some(v)) => v.get("name").and_then(|n| n.as_str()).map(String::from),
                    Ok(None) => {
                        warn!(
                            "Job {} - album {} not found in catalog for notification name",
                            job_id, album_id
                        );
                        None
                    }
                    Err(e) => {
                        warn!(
                            "Job {} - failed to look up album {} for notification name: {}",
                            job_id, album_id, e
                        );
                        None
                    }
                };

                let catalog_artist_name = match self.catalog.get_album_artist_ids(album_id) {
                    Ok(ids) => ids.into_iter().next().and_then(|aid| {
                        match self.catalog.get_artist_json(&aid) {
                            Ok(Some(v)) => {
                                v.get("name").and_then(|n| n.as_str()).map(String::from)
                            }
                            Ok(None) => {
                                warn!(
                                    "Job {} - artist {} not found in catalog for notification name",
                                    job_id, aid
                                );
                                None
                            }
                            Err(e) => {
                                warn!(
                                    "Job {} - failed to look up artist {} for notification name: {}",
                                    job_id, aid, e
                                );
                                None
                            }
                        }
                    }),
                    Err(e) => {
                        warn!(
                            "Job {} - failed to get artist IDs for album {} for notification name: {}",
                            job_id, album_id, e
                        );
                        None
                    }
                };

                let album_name = catalog_album_name
                    .or_else(|| download_request_album_name.clone())
                    .or_else(|| job.detected_album.clone())
                    .unwrap_or_else(|| {
                        warn!(
                            "Job {} - all album name sources exhausted, using Unknown Album",
                            job_id
                        );
                        "Unknown Album".to_string()
                    });

                let artist_name = catalog_artist_name
                    .or_else(|| download_request_artist_name.clone())
                    .or_else(|| job.detected_artist.clone())
                    .unwrap_or_else(|| {
                        warn!(
                            "Job {} - all artist name sources exhausted, using Unknown Artist",
                            job_id
                        );
                        "Unknown Artist".to_string()
                    });

                (album_name, artist_name)
            }
            None => {
                warn!(
                    "Job {} - no matched_album_id at notification time, using fallbacks",
                    job_id
                );
                (
                    download_request_album_name
                        .clone()
                        .or_else(|| job.detected_album.clone())
                        .unwrap_or_else(|| "Unknown Album".to_string()),
                    download_request_artist_name
                        .clone()
                        .or_else(|| job.detected_artist.clone())
                        .unwrap_or_else(|| "Unknown Artist".to_string()),
                )
            }
        };

        if let Some(notifier) = &self.notifier {
            notifier
                .notify_completed(&job, converted as u32, &album_name, &artist_name)
                .await;

            // Emit catalog invalidation event for the album
            if let Some(album_id) = &job.matched_album_id {
                notifier
                    .emit_catalog_event(
                        crate::server_store::CatalogEventType::AlbumUpdated,
                        crate::server_store::CatalogContentType::Album,
                        album_id,
                        "ingestion",
                    )
                    .await;
            }
        }

        // Send download-completed notifications to requesting users
        if let Some(album_id) = &job.matched_album_id {
            // Notify the primary requester
            if let (Some(user_id), Some(request_id)) =
                (&primary_requester_user_id, &primary_requester_request_id)
            {
                self.send_download_notification(
                    user_id,
                    request_id,
                    album_id,
                    &album_name,
                    &artist_name,
                )
                .await;
            }

            // Notify auto-completed requesters
            for completed in &auto_completed_requests {
                if let Some(user_id) = &completed.requested_by_user_id {
                    self.send_download_notification(
                        user_id,
                        &completed.id,
                        album_id,
                        &album_name,
                        &artist_name,
                    )
                    .await;
                }
            }
        }

        info!(
            "Completed job {} - converted {}/{} tracks",
            job_id, converted, job.tracks_matched
        );

        Ok(())
    }

    // =========================================================================
    // Review Handling
    // =========================================================================

    /// Resolve a review and continue processing.
    pub async fn resolve_review(
        &self,
        job_id: &str,
        reviewer_user_id: &str,
        selected_option: &str,
    ) -> Result<(), IngestionError> {
        let mut job = self
            .store
            .get_job(job_id)?
            .ok_or_else(|| IngestionError::JobNotFound(job_id.to_string()))?;

        if job.status != IngestionJobStatus::AwaitingReview {
            return Err(IngestionError::InvalidState {
                expected: "AWAITING_REVIEW".to_string(),
                actual: job.status.as_str().to_string(),
            });
        }

        self.store
            .resolve_review(job_id, reviewer_user_id, selected_option)?;

        if selected_option.starts_with("album:") {
            let album_id = selected_option.trim_start_matches("album:");
            job.matched_album_id = Some(album_id.to_string());
            job.match_confidence = Some(1.0);
            job.match_source = Some(IngestionMatchSource::HumanReview);
            job.status = IngestionJobStatus::MappingTracks;
            self.store.update_job(&job)?;

            info!(
                "Review resolved: job {} matched to album {}",
                job_id, album_id
            );

            // Continue to track mapping and conversion
            // Skip duration review since user already approved this album
            self.map_tracks(job_id, true).await?;

            // Fail if no tracks could be matched
            let job_after_map = self.store.get_job(job_id)?.unwrap();
            if job_after_map.tracks_matched == 0 {
                let mut job = job_after_map;
                self.fail_job_with_cleanup(
                    &mut job,
                    "No tracks could be matched — files may lack metadata tags or have corrupt audio data",
                ).await?;

                return Err(IngestionError::Store(anyhow::anyhow!(
                    "Zero tracks matched for job {}",
                    job_id
                )));
            }

            self.convert_job(job_id).await?;
        } else if selected_option == "no_match" {
            self.fail_job_with_cleanup(&mut job, "Album not in catalog")
                .await?;
        } else if selected_option == "continue" {
            // User accepted duration mismatches, continue to conversion
            job.status = IngestionJobStatus::Converting;
            self.store.update_job(&job)?;

            info!(
                "Review resolved: job {} continuing despite duration mismatches",
                job_id
            );

            self.convert_job(job_id).await?;
        } else if selected_option == "convert_low_bitrate" {
            // User approved converting low bitrate files
            let mut files = self.store.get_files_for_job(job_id)?;
            for file in &mut files {
                if let Some(ConversionReason::LowBitratePendingConfirmation { original_bitrate }) =
                    file.conversion_reason
                {
                    file.conversion_reason =
                        Some(ConversionReason::LowBitrateApproved { original_bitrate });
                    self.store.update_file(file)?;
                }
            }

            // Continue to identification phase
            job.status = IngestionJobStatus::IdentifyingAlbum;
            self.store.update_job(&job)?;

            info!(
                "Review resolved: job {} low bitrate files approved for conversion",
                job_id
            );

            // Continue processing
            self.process_job(job_id).await?;
        } else if selected_option == "retry" {
            job.status = IngestionJobStatus::IdentifyingAlbum;
            self.store.update_job(&job)?;
        }

        Ok(())
    }

    /// Get pending review items.
    pub fn get_pending_reviews(
        &self,
        limit: usize,
    ) -> Result<Vec<super::models::ReviewQueueItem>, IngestionError> {
        Ok(self.store.get_pending_reviews(limit)?)
    }

    /// Delete a job and its associated files.
    pub async fn delete_job(&self, job_id: &str) -> Result<(), IngestionError> {
        // Clean up temp files
        if let Err(e) = self.file_handler.cleanup_job(job_id).await {
            warn!("Failed to cleanup files for job {}: {}", job_id, e);
        }

        // Delete from database (cascades to files and review queue)
        self.store.delete_job(job_id)?;

        info!("Deleted job {}", job_id);
        Ok(())
    }
}

// =========================================================================
// Utilities
// =========================================================================

/// Calculate string similarity (0.0 to 1.0).
fn string_similarity(a: &str, b: &str) -> f32 {
    let a = a.to_lowercase();
    let b = b.to_lowercase();

    if a == b {
        return 1.0;
    }

    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    // Use Levenshtein distance
    let distance = levenshtein_distance(&a, &b);
    let max_len = a.len().max(b.len());

    1.0 - (distance as f32 / max_len as f32)
}

/// Parse metadata from review option label/description.
///
/// Returns (score, track_count, delta_ms) parsed from strings like:
/// - Label: "Artist - Album (75%, 12 tracks)"
/// - Description: "Match: 75%, Delta: 500ms, 12 tracks"
fn parse_option_metadata(label: &str, description: Option<&str>) -> (f32, i32, i64) {
    let mut score = 0.0f32;
    let mut track_count = 0i32;
    let mut delta_ms = 0i64;

    // Try to parse from description first (more structured)
    if let Some(desc) = description {
        // Format: "Match: XX%, Delta: Yms, Z tracks"
        if let Some(match_start) = desc.find("Match: ") {
            let rest = &desc[match_start + 7..];
            if let Some(pct_end) = rest.find('%') {
                if let Ok(pct) = rest[..pct_end].trim().parse::<f32>() {
                    score = pct / 100.0;
                }
            }
        }
        if let Some(delta_start) = desc.find("Delta: ") {
            let rest = &desc[delta_start + 7..];
            if let Some(ms_end) = rest.find("ms") {
                if let Ok(ms) = rest[..ms_end].trim().parse::<i64>() {
                    delta_ms = ms;
                }
            }
        }
        // Parse track count from "N tracks"
        for word in desc.split_whitespace() {
            if let Ok(n) = word.parse::<i32>() {
                // Check if next word is "tracks"
                if desc.contains(&format!("{} tracks", n)) {
                    track_count = n;
                    break;
                }
            }
        }
    }

    // Fallback: parse from label format "Artist - Album (XX%, N tracks)"
    if score == 0.0 {
        if let Some(paren_start) = label.rfind('(') {
            let in_parens = &label[paren_start + 1..];
            if let Some(pct_end) = in_parens.find('%') {
                if let Ok(pct) = in_parens[..pct_end].trim().parse::<f32>() {
                    score = pct / 100.0;
                }
            }
            // Parse track count
            for word in in_parens.split_whitespace() {
                if let Ok(n) = word.parse::<i32>() {
                    if in_parens.contains(&format!("{} tracks", n)) {
                        track_count = n;
                        break;
                    }
                }
            }
        }
    }

    (score, track_count, delta_ms)
}

/// Calculate Levenshtein distance between two strings.
#[allow(clippy::needless_range_loop)]
fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let m = a_chars.len();
    let n = b_chars.len();

    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }

    let mut dp = vec![vec![0usize; n + 1]; m + 1];

    for i in 0..=m {
        dp[i][0] = i;
    }
    for j in 0..=n {
        dp[0][j] = j;
    }

    for i in 1..=m {
        for j in 1..=n {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }

    dp[m][n]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = IngestionManagerConfig::default();
        assert_eq!(config.target_bitrate, 320);
        assert_eq!(config.bitrate_tolerance, 50);
        assert_eq!(config.max_iterations, 20);
        assert!((config.auto_match_threshold - 0.85).abs() < 0.001);
    }

    #[test]
    fn test_string_similarity() {
        assert!((string_similarity("Abbey Road", "Abbey Road") - 1.0).abs() < 0.001);
        assert!((string_similarity("abbey road", "Abbey Road") - 1.0).abs() < 0.001);
        assert!(string_similarity("Abbey Road", "The Beatles") < 0.5);
        // "Abbey Rd" vs "Abbey Road": distance=2, max_len=10 -> 0.8 similarity
        assert!(string_similarity("Abbey Rd", "Abbey Road") >= 0.8);
    }

    #[test]
    fn test_levenshtein_distance() {
        assert_eq!(levenshtein_distance("", ""), 0);
        assert_eq!(levenshtein_distance("abc", "abc"), 0);
        assert_eq!(levenshtein_distance("abc", ""), 3);
        assert_eq!(levenshtein_distance("", "abc"), 3);
        assert_eq!(levenshtein_distance("kitten", "sitting"), 3);
    }

    /// Test bitrate decision logic with default config (320 kbps ± 50 kbps)
    fn bitrate_to_conversion_reason(
        config: &IngestionManagerConfig,
        bitrate: Option<i32>,
    ) -> ConversionReason {
        let min_bitrate = config.target_bitrate as i32 - config.bitrate_tolerance as i32;
        let max_bitrate = config.target_bitrate as i32 + config.bitrate_tolerance as i32;

        let bitrate = match bitrate {
            Some(b) if b > 0 => b,
            _ => return ConversionReason::UndetectableBitrate,
        };

        if bitrate < min_bitrate {
            return ConversionReason::LowBitratePendingConfirmation {
                original_bitrate: bitrate,
            };
        }

        if bitrate > max_bitrate {
            return ConversionReason::HighBitrate {
                original_bitrate: bitrate,
            };
        }

        ConversionReason::NoConversionNeeded
    }

    #[test]
    fn test_bitrate_conversion_decision() {
        let config = IngestionManagerConfig::default();

        // Test undetectable bitrate (None)
        let result = bitrate_to_conversion_reason(&config, None);
        assert!(matches!(result, ConversionReason::UndetectableBitrate));

        // Test undetectable bitrate (Some(0))
        let result = bitrate_to_conversion_reason(&config, Some(0));
        assert!(matches!(result, ConversionReason::UndetectableBitrate));

        // Test low bitrate (< 270 kbps)
        let result = bitrate_to_conversion_reason(&config, Some(128));
        assert!(matches!(
            result,
            ConversionReason::LowBitratePendingConfirmation {
                original_bitrate: 128
            }
        ));

        // Test low bitrate at boundary (269 kbps)
        let result = bitrate_to_conversion_reason(&config, Some(269));
        assert!(matches!(
            result,
            ConversionReason::LowBitratePendingConfirmation {
                original_bitrate: 269
            }
        ));

        // Test acceptable bitrate (270 kbps - lower boundary)
        let result = bitrate_to_conversion_reason(&config, Some(270));
        assert!(matches!(result, ConversionReason::NoConversionNeeded));

        // Test acceptable bitrate (320 kbps - target)
        let result = bitrate_to_conversion_reason(&config, Some(320));
        assert!(matches!(result, ConversionReason::NoConversionNeeded));

        // Test acceptable bitrate (370 kbps - upper boundary)
        let result = bitrate_to_conversion_reason(&config, Some(370));
        assert!(matches!(result, ConversionReason::NoConversionNeeded));

        // Test high bitrate (371 kbps)
        let result = bitrate_to_conversion_reason(&config, Some(371));
        assert!(matches!(
            result,
            ConversionReason::HighBitrate {
                original_bitrate: 371
            }
        ));

        // Test high bitrate (500 kbps)
        let result = bitrate_to_conversion_reason(&config, Some(500));
        assert!(matches!(
            result,
            ConversionReason::HighBitrate {
                original_bitrate: 500
            }
        ));
    }
}
