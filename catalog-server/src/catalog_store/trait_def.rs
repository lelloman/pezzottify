//! CatalogStore trait definition.
//!
//! This trait abstracts catalog operations for the Spotify metadata database.
//! The database is primarily read-only (imported from Spotify dump).

use anyhow::Result;
use std::path::PathBuf;

/// Trait for catalog storage backends.
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

    /// Get a track by ID, returning the typed Track struct.
    fn get_track(&self, id: &str) -> Result<Option<super::Track>>;

    // =========================================================================
    // Resolved Entity Retrieval
    // =========================================================================

    /// Get a resolved artist with all related data.
    fn get_resolved_artist_json(&self, id: &str) -> Result<Option<serde_json::Value>>;

    /// Get a resolved album with all related data.
    fn get_resolved_album_json(&self, id: &str) -> Result<Option<serde_json::Value>>;

    /// Get a resolved track with all related data.
    fn get_resolved_track_json(&self, id: &str) -> Result<Option<serde_json::Value>>;

    /// Get a resolved artist with all related data (typed version).
    fn get_resolved_artist(&self, id: &str) -> Result<Option<super::ResolvedArtist>>;

    /// Get a resolved album with all related data (typed version).
    fn get_resolved_album(&self, id: &str) -> Result<Option<super::ResolvedAlbum>>;

    /// Get a resolved track with all related data (typed version).
    fn get_resolved_track(&self, id: &str) -> Result<Option<super::ResolvedTrack>>;

    /// Get an artist's discography.
    fn get_artist_discography_json(&self, id: &str) -> Result<Option<serde_json::Value>>;

    /// Get an artist's discography (typed version).
    fn get_discography(&self, id: &str) -> Result<Option<super::ArtistDiscography>>;

    // =========================================================================
    // Image URL Retrieval (Spotify CDN URLs)
    // =========================================================================

    /// Get the largest image URL for an album from album_images table.
    fn get_album_image_url(&self, album_id: &str) -> Result<Option<super::ImageUrl>>;

    /// Get the largest image URL for an artist from artist_images table.
    fn get_artist_image_url(&self, artist_id: &str) -> Result<Option<super::ImageUrl>>;

    // =========================================================================
    // File Path Resolution
    // =========================================================================

    /// Get the filesystem path to an image (for lazy-downloaded images).
    /// The id is the Spotify ID (album or artist).
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
    // Search Support
    // =========================================================================

    /// Get all searchable content for building the search index.
    fn get_searchable_content(&self) -> Result<Vec<SearchableItem>>;

    // =========================================================================
    // Integrity Support
    // =========================================================================

    /// List all track IDs in the catalog.
    fn list_all_track_ids(&self) -> Result<Vec<String>>;
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
