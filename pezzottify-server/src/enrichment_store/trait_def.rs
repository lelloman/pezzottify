//! EnrichmentStore trait definition.

use super::models::{AlbumEnrichment, ArtistEnrichment, AudioFeatures, EnrichmentStats};
use anyhow::Result;

/// Trait for enrichment storage backends.
pub trait EnrichmentStore: Send + Sync {
    // =========================================================================
    // Audio Features (tracks)
    // =========================================================================

    /// Get audio features for a track by ID.
    fn get_audio_features(&self, track_id: &str) -> Result<Option<AudioFeatures>>;

    /// Insert or update audio features for a track.
    fn upsert_audio_features(&self, features: &AudioFeatures) -> Result<()>;

    /// Insert or update audio features for multiple tracks in a single transaction.
    fn upsert_audio_features_batch(&self, features: &[AudioFeatures]) -> Result<()>;

    /// Get track IDs that exist in the catalog but not in audio_features.
    /// `catalog_track_ids` is the list of available track IDs from the catalog store.
    fn get_tracks_needing_analysis(&self, catalog_track_ids: &[String], limit: usize) -> Result<Vec<String>>;

    // =========================================================================
    // Artist Enrichment
    // =========================================================================

    /// Get enrichment data for an artist by ID.
    fn get_artist_enrichment(&self, artist_id: &str) -> Result<Option<ArtistEnrichment>>;

    /// Insert or update enrichment data for an artist.
    fn upsert_artist_enrichment(&self, enrichment: &ArtistEnrichment) -> Result<()>;

    /// Get artist IDs that exist in the catalog but not in artist_enrichment.
    /// `catalog_artist_ids` is the list of artist IDs from the catalog store.
    fn get_artists_needing_enrichment(&self, catalog_artist_ids: &[String], limit: usize) -> Result<Vec<String>>;

    // =========================================================================
    // Album Enrichment
    // =========================================================================

    /// Get enrichment data for an album by ID.
    fn get_album_enrichment(&self, album_id: &str) -> Result<Option<AlbumEnrichment>>;

    /// Insert or update enrichment data for an album.
    fn upsert_album_enrichment(&self, enrichment: &AlbumEnrichment) -> Result<()>;

    /// Get album IDs that exist in the catalog but not in album_enrichment.
    /// `catalog_album_ids` is the list of album IDs from the catalog store.
    fn get_albums_needing_enrichment(&self, catalog_album_ids: &[String], limit: usize) -> Result<Vec<String>>;

    // =========================================================================
    // Statistics
    // =========================================================================

    /// Get summary statistics for the enrichment database.
    fn get_enrichment_stats(&self) -> Result<EnrichmentStats>;
}
