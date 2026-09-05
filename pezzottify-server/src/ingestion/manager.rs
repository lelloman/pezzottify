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
use super::store::{IngestionStore, JobClaimResult};
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

    #[error("Job is already being processed: {0}")]
    JobBusy(String),

    #[error("Invalid ingestion context: {0}")]
    InvalidContext(String),

    #[error("No files in upload")]
    NoFiles,

    #[error("Album not matched")]
    AlbumNotMatched,
}

struct JobClaimGuard {
    store: Arc<dyn IngestionStore>,
    job_id: String,
}

impl Drop for JobClaimGuard {
    fn drop(&mut self) {
        if let Err(error) = self.store.release_job_claim(&self.job_id) {
            error!(job_id = %self.job_id, %error, "Failed to release ingestion job claim");
        }
    }
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
    media: Arc<crate::media::MediaManager>,
    config: IngestionManagerConfig,
    download_manager: Option<Arc<dyn DownloadManagerTrait>>,
    notifier: Option<Arc<super::notifier::IngestionNotifier>>,
    notification_service: Option<Arc<crate::notifications::NotificationService>>,
}

include!("setup.rs");
include!("upload.rs");
include!("fingerprint_workflow.rs");
include!("analysis.rs");
include!("processing.rs");
include!("mapping.rs");
include!("conversion.rs");
include!("reviews.rs");
include!("matching.rs");
