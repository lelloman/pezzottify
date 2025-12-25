//! Type definitions for downloader service API responses.
//!
//! Defines structs for deserializing responses from the external downloader service.
//! These types match the JSON structure returned by the downloader API.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Raw discography result from the downloader - just album IDs.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RawDiscographyResult {
    /// List of album IDs by this artist
    pub albums: Vec<String>,
}

// =============================================================================
// Metadata Types
// =============================================================================

/// Album metadata from the external downloader service.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExternalAlbum {
    /// Album ID
    pub id: String,
    /// Album name
    pub name: String,
    /// Album type: "album", "single", "ep", "compilation"
    pub album_type: String,
    /// IDs of artists on this album
    #[serde(default)]
    pub artists_ids: Vec<String>,
    /// Record label
    #[serde(default)]
    pub label: String,
    /// Release date as Unix timestamp
    #[serde(default)]
    pub date: i64,
    /// Genre tags
    #[serde(default)]
    pub genres: Vec<String>,
    /// Cover images in various sizes
    #[serde(default)]
    pub covers: Vec<ExternalImage>,
    /// Disc information with track listings
    #[serde(default)]
    pub discs: Vec<ExternalDisc>,
    /// Related album IDs
    #[serde(default)]
    pub related: Vec<String>,
    /// Cover images (alternative to covers)
    #[serde(default)]
    pub cover_group: Vec<ExternalImage>,
    /// Original title
    #[serde(default)]
    pub original_title: Option<String>,
    /// Version title
    #[serde(default)]
    pub version_title: String,
    /// Type string
    #[serde(default)]
    pub type_str: String,
}

/// Disc information within an album.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExternalDisc {
    /// Disc number (1-indexed)
    pub number: i32,
    /// Disc name (often empty)
    pub name: String,
    /// Track IDs on this disc
    pub tracks: Vec<String>,
}

/// Image metadata from the external downloader service.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExternalImage {
    /// Hex ID for downloading this image
    pub id: String,
    /// Size category: "small", "medium", "large", "xlarge"
    pub size: String,
    /// Image width in pixels
    pub width: i32,
    /// Image height in pixels
    pub height: i32,
}

/// Artist with role on a track.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExternalArtistWithRole {
    pub artist_id: String,
    pub name: String,
    pub role: String,
}

/// Track metadata from the external downloader service.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExternalTrack {
    /// Track ID
    pub id: String,
    /// Track name
    pub name: String,
    /// ID of the album this track belongs to
    pub album_id: String,
    /// IDs of artists on this track
    pub artists_ids: Vec<String>,
    /// Track number within the disc
    pub number: i32,
    /// Disc number
    pub disc_number: i32,
    /// Duration in milliseconds
    pub duration: i64,
    /// Whether the track has explicit content
    #[serde(default)]
    pub is_explicit: bool,
    /// Available audio files (format -> file hash)
    #[serde(default)]
    pub files: HashMap<String, String>,
    /// Alternative track IDs
    #[serde(default)]
    pub alternatives: Vec<String>,
    /// Tags
    #[serde(default)]
    pub tags: Vec<String>,
    /// Earliest live timestamp
    #[serde(default)]
    pub earliest_live_timestamp: Option<i64>,
    /// Whether the track has lyrics
    #[serde(default)]
    pub has_lyrics: bool,
    /// Languages of performance
    #[serde(default)]
    pub language_of_performance: Vec<String>,
    /// Original title
    #[serde(default)]
    pub original_title: Option<String>,
    /// Version title
    #[serde(default)]
    pub version_title: String,
    /// Artists with their roles
    #[serde(default)]
    pub artists_with_role: Vec<ExternalArtistWithRole>,
}

/// Artist metadata from the external downloader service.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExternalArtist {
    /// Artist ID
    pub id: String,
    /// Artist name
    pub name: String,
    /// Genre tags
    #[serde(default)]
    pub genre: Vec<String>,
    /// Profile images in various sizes
    #[serde(default)]
    pub portraits: Vec<ExternalImage>,
    /// Activity periods
    #[serde(default)]
    pub activity_periods: Vec<serde_json::Value>,
    /// Related artist IDs
    #[serde(default)]
    pub related: Vec<String>,
    /// Portrait images (alternative to portraits)
    #[serde(default)]
    pub portrait_group: Vec<ExternalImage>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_external_album_deserialize() {
        let json = r#"{
            "id": "album123",
            "name": "Test Album",
            "album_type": "album",
            "artists_ids": ["artist1", "artist2"],
            "label": "Test Label",
            "date": 1704067200,
            "genres": ["rock", "pop"],
            "covers": [
                {"id": "img1", "size": "large", "width": 640, "height": 640}
            ],
            "discs": [
                {"number": 1, "name": "", "tracks": ["track1", "track2"]}
            ]
        }"#;

        let album: ExternalAlbum = serde_json::from_str(json).unwrap();
        assert_eq!(album.id, "album123");
        assert_eq!(album.name, "Test Album");
        assert_eq!(album.artists_ids.len(), 2);
        assert_eq!(album.covers.len(), 1);
        assert_eq!(album.discs.len(), 1);
        assert_eq!(album.discs[0].tracks.len(), 2);
    }

    #[test]
    fn test_external_track_deserialize() {
        let json = r#"{
            "id": "track123",
            "name": "Test Track",
            "album_id": "album123",
            "artists_ids": ["artist1"],
            "number": 1,
            "disc_number": 1,
            "duration": 180000,
            "is_explicit": false
        }"#;

        let track: ExternalTrack = serde_json::from_str(json).unwrap();
        assert_eq!(track.id, "track123");
        assert_eq!(track.name, "Test Track");
        assert_eq!(track.duration, 180000);
        assert!(!track.is_explicit);
    }

    #[test]
    fn test_external_artist_deserialize() {
        let json = r#"{
            "id": "artist123",
            "name": "Test Artist",
            "genre": ["rock"],
            "portraits": []
        }"#;

        let artist: ExternalArtist = serde_json::from_str(json).unwrap();
        assert_eq!(artist.id, "artist123");
        assert_eq!(artist.name, "Test Artist");
        assert_eq!(artist.genre.len(), 1);
    }
}
