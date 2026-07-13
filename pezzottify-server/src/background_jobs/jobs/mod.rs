//! Specific background job implementations.
//!
//! This module contains implementations of the `BackgroundJob` trait
//! for various server maintenance and processing tasks.

// TODO: Re-enable after updating for Spotify schema
// pub mod audit_log_cleanup;
// pub mod expand_artists_base;
// pub mod missing_files_watchdog;
pub mod album_embedding_sync;
pub mod audio_analysis;
pub mod catalog_availability_stats;
pub mod catalog_cardinality_stats;
pub mod device_pruning;
pub mod featured_albums;
pub mod ingestion_cleanup;
pub mod metadata_enrichment;
pub mod popular_content;
pub mod related_artists_enrichment;
pub mod track_embedding_sync;
pub mod whatsnew_batch;

// pub use audit_log_cleanup::AuditLogCleanupJob;
// pub use expand_artists_base::ExpandArtistsBaseJob;
// pub use missing_files_watchdog::MissingFilesWatchdogJob;
pub use album_embedding_sync::AlbumEmbeddingSyncJob;
pub use audio_analysis::AudioAnalysisJob;
pub use catalog_availability_stats::{
    CatalogAvailabilityStatsJob, CatalogAvailabilityStatsSnapshot,
};
pub use catalog_cardinality_stats::CatalogCardinalityStatsJob;
pub use device_pruning::DevicePruningJob;
pub use featured_albums::{FeaturedAlbum, FeaturedAlbumsJob, FeaturedAlbumsSnapshot};
pub use ingestion_cleanup::IngestionCleanupJob;
pub use metadata_enrichment::MetadataEnrichmentJob;
pub use popular_content::PopularContentJob;
pub use related_artists_enrichment::RelatedArtistsEnrichmentJob;
pub use track_embedding_sync::TrackEmbeddingSyncJob;
pub use whatsnew_batch::WhatsNewBatchJob;
