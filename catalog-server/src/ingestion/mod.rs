//! Agentic ingestion feature for uploading and matching audio files.
//!
//! Album-first ingestion workflow:
//! 1. User uploads a zip containing audio files
//! 2. System extracts and analyzes all files (embedded tags, duration)
//! 3. Duration fingerprint matching identifies the album
//! 4. Once album is confirmed, files are mapped to tracks
//! 5. Matched files are converted to OGG Vorbis and stored

mod converter;
mod file_handler;
mod fingerprint;
mod manager;
mod models;
mod notifier;
mod schema;
mod store;
mod tools;

pub use converter::{convert_to_ogg, probe_audio_file, AudioMetadata, ConversionError};
pub use file_handler::{FileHandler, FileHandlerError};
pub use fingerprint::{
    match_album_by_fingerprint, match_album_with_fallbacks, FingerprintConfig,
    FingerprintMatchResult, ScoredCandidate,
};
pub use manager::{
    AlbumCandidateInfo, DownloadManagerTrait, IngestionError, IngestionManager,
    IngestionManagerConfig, QueueItemInfo, UploadResult,
};
pub use models::{
    AlbumMetadataSummary, ConversionReason, IngestionContextType, IngestionFile, IngestionJob,
    IngestionJobStatus, IngestionMatchSource, ReviewOption, ReviewQueueItem, TicketType,
    UploadType,
};
pub use notifier::IngestionNotifier;
pub use schema::{INGESTION_SCHEMA_SQL, INGESTION_SCHEMA_VERSION};
pub use store::{IngestionStore, SqliteIngestionStore};
pub use tools::create_ingestion_tools;
