//! Specific background job implementations.
//!
//! This module contains implementations of the `BackgroundJob` trait
//! for various server maintenance and processing tasks.

// TODO: Re-enable after updating for Spotify schema
// pub mod audit_log_cleanup;
// pub mod expand_artists_base;
// pub mod missing_files_watchdog;
pub mod audio_analysis;
pub mod catalog_availability_stats;
pub mod device_pruning;
pub mod ingestion_cleanup;
pub mod popular_content;
pub mod related_artists_enrichment;
pub mod whatsnew_batch;

// pub use audit_log_cleanup::AuditLogCleanupJob;
// pub use expand_artists_base::ExpandArtistsBaseJob;
// pub use missing_files_watchdog::MissingFilesWatchdogJob;
pub use audio_analysis::AudioAnalysisJob;
pub use catalog_availability_stats::{
    CatalogAvailabilityStatsJob, CatalogAvailabilityStatsSnapshot,
};
pub use device_pruning::DevicePruningJob;
pub use ingestion_cleanup::IngestionCleanupJob;
pub use popular_content::PopularContentJob;
pub use related_artists_enrichment::RelatedArtistsEnrichmentJob;
pub use whatsnew_batch::WhatsNewBatchJob;
