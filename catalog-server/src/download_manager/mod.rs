//! Download Manager module
//!
//! Provides a queue-based download request system. Users submit download requests
//! which are stored passively in a queue. An external script picks up pending
//! requests and fulfills them via the ingestion system.

mod audit_logger;
mod manager;
mod models;
mod queue_store;
mod retry_policy;
mod schema;
mod sync_notifier;
mod watchdog;

pub use audit_logger::AuditLogger;
pub use manager::DownloadManager;
pub use models::*;
pub use queue_store::{DownloadQueueStore, SqliteDownloadQueueStore};
pub use retry_policy::RetryPolicy;
pub use schema::DOWNLOAD_QUEUE_VERSIONED_SCHEMAS;
pub use sync_notifier::DownloadSyncNotifier;
pub use watchdog::MissingFilesWatchdog;
