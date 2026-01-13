//! Agentic ingestion feature for uploading and matching audio files.
//!
//! Album-first ingestion workflow:
//! 1. User uploads a zip containing audio files
//! 2. System extracts and analyzes all files (embedded tags, duration)
//! 3. Agent identifies the album from collective metadata
//! 4. Once album is confirmed, files are mapped to tracks
//! 5. Matched files are converted to OGG Vorbis and stored

mod converter;
mod file_handler;
mod manager;
mod models;
mod schema;
mod store;
mod tools;

pub use converter::{convert_to_ogg, probe_audio_file, AudioMetadata, ConversionError};
pub use file_handler::{FileHandler, FileHandlerError};
pub use manager::{IngestionError, IngestionManager, IngestionManagerConfig};
pub use models::{
    AlbumMetadataSummary, IngestionContextType, IngestionFile, IngestionJob, IngestionJobStatus,
    IngestionMatchSource, ReviewOption, ReviewQueueItem,
};
pub use schema::{INGESTION_SCHEMA_SQL, INGESTION_SCHEMA_VERSION};
pub use store::{IngestionStore, SqliteIngestionStore};
pub use tools::create_ingestion_tools;
