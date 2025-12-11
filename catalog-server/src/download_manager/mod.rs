//! Download Manager module
//!
//! Provides a queue-based asynchronous download manager that handles content
//! downloads from an external downloader service.

mod audit_logger;
mod catalog_ingestion;
mod corruption_handler;
mod downloader_client;
mod downloader_types;
mod job_processor;
mod manager;
mod models;
mod queue_store;
mod retry_policy;
mod schema;
mod search_proxy;
mod throttle;
mod watchdog;

pub use audit_logger::AuditLogger;
pub use corruption_handler::{
    CorruptionHandler, CorruptionHandlerConfig, HandlerAction, HandlerState,
    PersistedState as CorruptionPersistedState,
};
pub use downloader_client::DownloaderClient;
pub use job_processor::QueueProcessor;
pub use manager::DownloadManager;
pub use models::*;
pub use queue_store::{DownloadQueueStore, SqliteDownloadQueueStore};
pub use retry_policy::RetryPolicy;
pub use schema::DOWNLOAD_QUEUE_VERSIONED_SCHEMAS;
pub use throttle::{
    DownloadThrottler, NoOpThrottler, SlidingWindowThrottler, ThrottleStats, ThrottlerConfig,
};
pub use watchdog::IntegrityWatchdog;
