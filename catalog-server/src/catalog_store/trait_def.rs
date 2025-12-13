//! CatalogStore trait definition.
//!
//! This trait abstracts catalog operations to support both the legacy
//! in-memory Catalog and the new SqliteCatalogStore implementations.

use anyhow::Result;
use std::path::PathBuf;

/// Trait for catalog storage backends.
///
/// This allows the server to work with either the legacy in-memory `Catalog`
/// or the new `SqliteCatalogStore` transparently.
pub trait CatalogStore: Send + Sync {
    // =========================================================================
    // Basic Entity Retrieval
    // =========================================================================

    /// Get an artist by ID, returning the serialized JSON representation.
    fn get_artist_json(&self, id: &str) -> Result<Option<serde_json::Value>>;

    /// Get an album by ID, returning the serialized JSON representation.
    fn get_album_json(&self, id: &str) -> Result<Option<serde_json::Value>>;

    /// Get a track by ID, returning the serialized JSON representation.
    fn get_track_json(&self, id: &str) -> Result<Option<serde_json::Value>>;

    // =========================================================================
    // Resolved Entity Retrieval
    // =========================================================================

    /// Get a resolved artist with all related data (including related artists).
    fn get_resolved_artist_json(&self, id: &str) -> Result<Option<serde_json::Value>>;

    /// Get a resolved album with all related data.
    fn get_resolved_album_json(&self, id: &str) -> Result<Option<serde_json::Value>>;

    /// Get a resolved track with all related data.
    fn get_resolved_track_json(&self, id: &str) -> Result<Option<serde_json::Value>>;

    /// Get an artist's discography.
    fn get_artist_discography_json(&self, id: &str) -> Result<Option<serde_json::Value>>;

    // =========================================================================
    // File Path Resolution
    // =========================================================================

    /// Get the filesystem path to an image.
    fn get_image_path(&self, id: &str) -> PathBuf;

    /// Get the filesystem path to a track's audio file.
    fn get_track_audio_path(&self, track_id: &str) -> Option<PathBuf>;

    /// Get the album ID for a track (needed for audio path resolution).
    fn get_track_album_id(&self, track_id: &str) -> Option<String>;

    // =========================================================================
    // Counts (for metrics)
    // =========================================================================

    /// Get the number of artists in the catalog.
    fn get_artists_count(&self) -> usize;

    /// Get the number of albums in the catalog.
    fn get_albums_count(&self) -> usize;

    /// Get the number of tracks in the catalog.
    fn get_tracks_count(&self) -> usize;

    // =========================================================================
    // Search Support (iteration for building search index)
    // =========================================================================

    /// Get all searchable content for building the search index.
    /// Returns a vector of (id, name, content_type) tuples.
    fn get_searchable_content(&self) -> Result<Vec<SearchableItem>>;

    // =========================================================================
    // Write Operations
    // =========================================================================

    /// Create a new artist. Returns the created artist as JSON.
    fn create_artist(&self, data: serde_json::Value) -> Result<serde_json::Value>;

    /// Update an existing artist. Returns the updated artist as JSON.
    fn update_artist(&self, id: &str, data: serde_json::Value) -> Result<serde_json::Value>;

    /// Delete an artist by ID.
    fn delete_artist(&self, id: &str) -> Result<()>;

    /// Create a new album. Returns the created album as JSON.
    fn create_album(&self, data: serde_json::Value) -> Result<serde_json::Value>;

    /// Update an existing album. Returns the updated album as JSON.
    fn update_album(&self, id: &str, data: serde_json::Value) -> Result<serde_json::Value>;

    /// Delete an album by ID.
    fn delete_album(&self, id: &str) -> Result<()>;

    /// Create a new track. Returns the created track as JSON.
    fn create_track(&self, data: serde_json::Value) -> Result<serde_json::Value>;

    /// Update an existing track. Returns the updated track as JSON.
    fn update_track(&self, id: &str, data: serde_json::Value) -> Result<serde_json::Value>;

    /// Delete a track by ID.
    fn delete_track(&self, id: &str) -> Result<()>;

    /// Create a new image. Returns the created image as JSON.
    fn create_image(&self, data: serde_json::Value) -> Result<serde_json::Value>;

    /// Update an existing image. Returns the updated image as JSON.
    fn update_image(&self, id: &str, data: serde_json::Value) -> Result<serde_json::Value>;

    /// Delete an image by ID.
    fn delete_image(&self, id: &str) -> Result<()>;

    // =========================================================================
    // Changelog Operations
    // =========================================================================

    /// Create a new changelog batch. Returns error if a batch is already active.
    fn create_changelog_batch(
        &self,
        name: &str,
        description: Option<&str>,
    ) -> Result<super::CatalogBatch>;

    /// Get a changelog batch by ID.
    fn get_changelog_batch(&self, id: &str) -> Result<Option<super::CatalogBatch>>;

    /// Get the currently active (open) changelog batch, if any.
    fn get_active_changelog_batch(&self) -> Result<Option<super::CatalogBatch>>;

    /// Close a changelog batch by ID.
    fn close_changelog_batch(&self, id: &str) -> Result<()>;

    /// List changelog batches, optionally filtered by open/closed state.
    fn list_changelog_batches(&self, is_open: Option<bool>) -> Result<Vec<super::CatalogBatch>>;

    /// Delete a changelog batch. Only succeeds if the batch has no changes.
    fn delete_changelog_batch(&self, id: &str) -> Result<()>;

    /// Get all changes recorded in a batch.
    fn get_changelog_batch_changes(&self, batch_id: &str) -> Result<Vec<super::ChangeEntry>>;

    /// Get the change history for a specific entity.
    fn get_changelog_entity_history(
        &self,
        entity_type: super::ChangeEntityType,
        entity_id: &str,
    ) -> Result<Vec<super::ChangeEntry>>;

    /// Get closed batches with summaries for the What's New endpoint.
    fn get_whats_new_batches(&self, limit: usize) -> Result<Vec<super::WhatsNewBatch>>;

    /// Get batches that have been open longer than the specified threshold.
    /// Used for stale batch alerting.
    fn get_stale_batches(&self, stale_threshold_hours: u64) -> Result<Vec<super::CatalogBatch>>;

    /// Close all stale batches (inactive for longer than the configured threshold).
    /// Returns the number of batches closed.
    fn close_stale_batches(&self) -> Result<usize>;

    // =========================================================================
    // Integrity Watchdog Support
    // =========================================================================

    /// List all track IDs in the catalog.
    /// Used by the integrity watchdog to scan for missing audio files.
    fn list_all_track_ids(&self) -> Result<Vec<String>>;

    /// List all unique image IDs referenced by albums (from album_images table).
    /// Used by the integrity watchdog to scan for missing album cover images.
    fn list_all_album_image_ids(&self) -> Result<Vec<String>>;

    /// List all unique image IDs referenced by artists (from artist_images table).
    /// Used by the integrity watchdog to scan for missing artist portrait images.
    fn list_all_artist_image_ids(&self) -> Result<Vec<String>>;

    // =========================================================================
    // Image Relationship Operations
    // =========================================================================

    /// Add an image to an artist at the given position.
    fn add_artist_image(
        &self,
        artist_id: &str,
        image_id: &str,
        image_type: &super::ImageType,
        position: i32,
    ) -> Result<()>;

    /// Add an image to an album at the given position.
    fn add_album_image(
        &self,
        album_id: &str,
        image_id: &str,
        image_type: &super::ImageType,
        position: i32,
    ) -> Result<()>;

    /// Set the display image for an artist (the "best" image to show).
    fn set_artist_display_image(&self, artist_id: &str, image_id: &str) -> Result<()>;

    /// Set the display image for an album (the "best" image to show).
    fn set_album_display_image(&self, album_id: &str, image_id: &str) -> Result<()>;

    // =========================================================================
    // Skeleton Sync Operations
    // =========================================================================

    /// Get the current skeleton version.
    fn get_skeleton_version(&self) -> Result<i64>;

    /// Get the skeleton checksum, calculating if not cached.
    fn get_skeleton_checksum(&self) -> Result<String>;

    /// Get skeleton events since a given sequence number.
    fn get_skeleton_events_since(&self, seq: i64) -> Result<Vec<crate::skeleton::SkeletonEvent>>;

    /// Get the earliest available skeleton event sequence number.
    fn get_skeleton_earliest_seq(&self) -> Result<i64>;

    /// Get the latest skeleton event sequence number.
    fn get_skeleton_latest_seq(&self) -> Result<i64>;

    /// Get all artist IDs for skeleton sync.
    fn get_all_artist_ids(&self) -> Result<Vec<String>>;

    /// Get all albums with their artist IDs for skeleton sync.
    fn get_all_albums_skeleton(&self) -> Result<Vec<crate::skeleton::SkeletonAlbumEntry>>;

    /// Get all tracks with their album IDs for skeleton sync.
    fn get_all_tracks_skeleton(&self) -> Result<Vec<crate::skeleton::SkeletonTrackEntry>>;
}

/// A searchable item for the search index.
#[derive(Debug, Clone)]
pub struct SearchableItem {
    pub id: String,
    pub name: String,
    pub content_type: SearchableContentType,
    /// Additional searchable text (genres, etc.)
    pub additional_text: Vec<String>,
}

/// Type of searchable content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchableContentType {
    Artist,
    Album,
    Track,
}

/// Extension trait for catalog stores that support typed write operations.
///
/// This trait extends `CatalogStore` with methods that accept typed structs
/// rather than JSON values, which is needed for catalog ingestion from
/// the download manager.
pub trait WritableCatalogStore: CatalogStore {
    // =========================================================================
    // Existence Checks
    // =========================================================================

    /// Check if an artist exists in the catalog.
    fn artist_exists(&self, id: &str) -> Result<bool>;

    /// Check if an album exists in the catalog.
    fn album_exists(&self, id: &str) -> Result<bool>;

    /// Check if a track exists in the catalog.
    fn track_exists(&self, id: &str) -> Result<bool>;

    /// Check if an image exists in the catalog.
    fn image_exists(&self, id: &str) -> Result<bool>;

    // =========================================================================
    // Typed Insert Operations
    // =========================================================================

    /// Insert an artist into the catalog.
    fn insert_artist(&self, artist: &super::Artist) -> Result<()>;

    /// Insert an album into the catalog.
    fn insert_album(&self, album: &super::Album) -> Result<()>;

    /// Insert a track into the catalog.
    fn insert_track(&self, track: &super::Track) -> Result<()>;

    /// Insert an image into the catalog.
    fn insert_image(&self, image: &super::Image) -> Result<()>;

    // =========================================================================
    // Relationship Operations
    // =========================================================================

    /// Add an artist to an album at the given position.
    fn add_album_artist(&self, album_id: &str, artist_id: &str, position: i32) -> Result<()>;

    /// Add an artist to a track with a role at the given position.
    fn add_track_artist(
        &self,
        track_id: &str,
        artist_id: &str,
        role: &super::ArtistRole,
        position: i32,
    ) -> Result<()>;

    /// Update a track's audio URI and format after download.
    fn update_track_audio(
        &self,
        track_id: &str,
        audio_uri: &str,
        format: &super::Format,
    ) -> Result<()>;
}
