//! Specific background job implementations.
//!
//! This module contains implementations of the `BackgroundJob` trait
//! for various server maintenance and processing tasks.

pub mod audit_log_cleanup;
pub mod expand_artists_base;
pub mod missing_files_watchdog;
pub mod popular_content;

pub use audit_log_cleanup::AuditLogCleanupJob;
pub use expand_artists_base::ExpandArtistsBaseJob;
pub use missing_files_watchdog::MissingFilesWatchdogJob;
pub use popular_content::PopularContentJob;
