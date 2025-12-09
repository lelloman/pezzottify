//! Download Manager module
//!
//! Provides a queue-based asynchronous download manager that handles content
//! downloads from an external downloader service.

mod audit_logger;
mod catalog_ingestion;
mod downloader_client;
mod downloader_types;
mod job_processor;
mod manager;
mod models;
mod queue_store;
mod retry_policy;
mod schema;
mod search_proxy;
mod watchdog;

pub use manager::DownloadManager;
// pub use models::*;  // TODO: uncomment when models are implemented
pub use queue_store::{DownloadQueueStore, SqliteDownloadQueueStore};
pub use schema::DOWNLOAD_QUEUE_VERSIONED_SCHEMAS;
