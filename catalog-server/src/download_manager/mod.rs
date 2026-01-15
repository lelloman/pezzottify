//! Download Manager module
//!
//! Provides a queue-based asynchronous download manager that integrates with
//! Quentin Torrentino for content acquisition via torrents.

mod audit_logger;
mod job_processor;
mod manager;
mod models;
mod queue_store;
mod retry_policy;
mod schema;
mod sync_notifier;
mod torrent_client;
mod torrent_types;
mod watchdog;

pub use audit_logger::AuditLogger;
pub use job_processor::QueueProcessor;
pub use manager::DownloadManager;
pub use models::*;
pub use queue_store::{DownloadQueueStore, SqliteDownloadQueueStore};
pub use retry_policy::RetryPolicy;
pub use schema::DOWNLOAD_QUEUE_VERSIONED_SCHEMAS;
pub use sync_notifier::DownloadSyncNotifier;
pub use torrent_client::{TorrentClient, TorrentClientTrait};
pub use torrent_types::*;
pub use watchdog::MissingFilesWatchdog;
