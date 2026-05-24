//! EnrichmentStore trait definition.

use super::models::{
    AlbumEnrichment, AlbumEnrichmentV1, ArtistEnrichment, ArtistEnrichmentV1, AudioFeatures,
    EnrichmentQueueItemV1, EnrichmentStats, EntityAliasV1, EntityContributorV1,
    EntityEnrichmentStatusV1, EntityEvidenceV1, EntityExternalIdV1, EntityRelationV1,
    EntitySourceV1, EntityTagV1, TrackEnrichmentV1,
};
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
    fn get_tracks_needing_analysis(
        &self,
        catalog_track_ids: &[String],
        limit: usize,
    ) -> Result<Vec<String>>;

    // =========================================================================
    // Artist Enrichment
    // =========================================================================

    /// Get enrichment data for an artist by ID.
    fn get_artist_enrichment(&self, artist_id: &str) -> Result<Option<ArtistEnrichment>>;

    /// Insert or update enrichment data for an artist.
    fn upsert_artist_enrichment(&self, enrichment: &ArtistEnrichment) -> Result<()>;

    /// Get artist IDs that exist in the catalog but not in artist_enrichment.
    /// `catalog_artist_ids` is the list of artist IDs from the catalog store.
    fn get_artists_needing_enrichment(
        &self,
        catalog_artist_ids: &[String],
        limit: usize,
    ) -> Result<Vec<String>>;

    // =========================================================================
    // Album Enrichment
    // =========================================================================

    /// Get enrichment data for an album by ID.
    fn get_album_enrichment(&self, album_id: &str) -> Result<Option<AlbumEnrichment>>;

    /// Insert or update enrichment data for an album.
    fn upsert_album_enrichment(&self, enrichment: &AlbumEnrichment) -> Result<()>;

    /// Get album IDs that exist in the catalog but not in album_enrichment.
    /// `catalog_album_ids` is the list of album IDs from the catalog store.
    fn get_albums_needing_enrichment(
        &self,
        catalog_album_ids: &[String],
        limit: usize,
    ) -> Result<Vec<String>>;

    // =========================================================================
    // Versioned Metadata Enrichment v1
    // =========================================================================

    fn get_artist_enrichment_v1(&self, artist_id: &str) -> Result<Option<ArtistEnrichmentV1>>;
    fn upsert_artist_enrichment_v1(&self, enrichment: &ArtistEnrichmentV1) -> Result<()>;

    fn get_album_enrichment_v1(&self, album_id: &str) -> Result<Option<AlbumEnrichmentV1>>;
    fn upsert_album_enrichment_v1(&self, enrichment: &AlbumEnrichmentV1) -> Result<()>;

    fn get_track_enrichment_v1(&self, track_id: &str) -> Result<Option<TrackEnrichmentV1>>;
    fn upsert_track_enrichment_v1(&self, enrichment: &TrackEnrichmentV1) -> Result<()>;

    fn is_enrichment_missing_or_stale(
        &self,
        entity_type: &str,
        entity_id: &str,
        stale_after_secs: i64,
        now: i64,
    ) -> Result<bool>;

    fn enqueue_enrichment_if_missing_or_stale(
        &self,
        entity_type: &str,
        entity_id: &str,
        reason: &str,
        priority: i64,
        stale_after_secs: i64,
    ) -> Result<bool>;

    fn get_enrichment_queue_item(
        &self,
        entity_type: &str,
        entity_id: &str,
    ) -> Result<Option<EnrichmentQueueItemV1>>;

    fn claim_enrichment_queue_batch(&self, limit: usize) -> Result<Vec<EnrichmentQueueItemV1>>;
    fn complete_enrichment_queue_item(&self, id: i64) -> Result<()>;
    fn fail_enrichment_queue_item(
        &self,
        id: i64,
        error: &str,
        retry_after_secs: Option<i64>,
    ) -> Result<()>;

    fn get_entity_enrichment_status(
        &self,
        entity_type: &str,
        entity_id: &str,
    ) -> Result<Option<EntityEnrichmentStatusV1>>;

    fn replace_entity_tags(
        &self,
        entity_type: &str,
        entity_id: &str,
        tags: &[EntityTagV1],
    ) -> Result<()>;
    fn list_entity_tags(&self, entity_type: &str, entity_id: &str) -> Result<Vec<EntityTagV1>>;

    fn replace_entity_contributors(
        &self,
        entity_type: &str,
        entity_id: &str,
        contributors: &[EntityContributorV1],
    ) -> Result<()>;
    fn list_entity_contributors(
        &self,
        entity_type: &str,
        entity_id: &str,
    ) -> Result<Vec<EntityContributorV1>>;

    fn replace_entity_relations(
        &self,
        entity_type: &str,
        entity_id: &str,
        relations: &[EntityRelationV1],
    ) -> Result<()>;
    fn list_visible_entity_relations(
        &self,
        entity_type: &str,
        entity_id: &str,
        min_confidence: f64,
    ) -> Result<Vec<EntityRelationV1>>;

    fn replace_entity_sources(
        &self,
        entity_type: &str,
        entity_id: &str,
        sources: &[EntitySourceV1],
    ) -> Result<()>;

    fn replace_entity_aliases(
        &self,
        entity_type: &str,
        entity_id: &str,
        aliases: &[EntityAliasV1],
    ) -> Result<()>;

    fn replace_entity_external_ids(
        &self,
        entity_type: &str,
        entity_id: &str,
        external_ids: &[EntityExternalIdV1],
    ) -> Result<()>;

    fn replace_entity_evidence(
        &self,
        entity_type: &str,
        entity_id: &str,
        evidence: &[EntityEvidenceV1],
    ) -> Result<()>;

    // =========================================================================
    // Statistics
    // =========================================================================

    /// Get summary statistics for the enrichment database.
    fn get_enrichment_stats(&self) -> Result<EnrichmentStats>;
}
