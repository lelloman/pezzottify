//! Catalog models for Spotify-schema SQLite storage.
//!
//! These models are designed to work with the Spotify database schema
//! where primary keys are integer rowids with unique text Spotify IDs.

use serde::{Deserialize, Serialize};

// =============================================================================
// Enumerations
// =============================================================================

/// Artist role on a track (integer-backed for DB storage)
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[repr(i32)]
pub enum ArtistRole {
    MainArtist = 0,
    FeaturedArtist = 1,
    Composer = 2,
    Remixer = 3,
    Conductor = 4,
    Orchestra = 5,
}

impl ArtistRole {
    /// Convert from database integer representation
    pub fn from_db_int(i: i32) -> Self {
        match i {
            0 => ArtistRole::MainArtist,
            1 => ArtistRole::FeaturedArtist,
            2 => ArtistRole::Composer,
            3 => ArtistRole::Remixer,
            4 => ArtistRole::Conductor,
            5 => ArtistRole::Orchestra,
            _ => ArtistRole::MainArtist, // Default fallback
        }
    }

    /// Convert to database integer representation
    pub fn to_db_int(self) -> i32 {
        self as i32
    }
}

/// Album type classification
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AlbumType {
    Album,
    Single,
    Compilation,
    #[serde(rename = "appears_on")]
    AppearsOn,
}

impl AlbumType {
    /// Convert from database string representation (Spotify uses lowercase)
    pub fn from_db_str(s: &str) -> Self {
        match s {
            "album" => AlbumType::Album,
            "single" => AlbumType::Single,
            "compilation" => AlbumType::Compilation,
            "appears_on" => AlbumType::AppearsOn,
            _ => AlbumType::Album, // Default fallback
        }
    }

    /// Convert to database string representation
    pub fn to_db_str(&self) -> &'static str {
        match self {
            AlbumType::Album => "album",
            AlbumType::Single => "single",
            AlbumType::Compilation => "compilation",
            AlbumType::AppearsOn => "appears_on",
        }
    }
}

// =============================================================================
// Core Entities
// =============================================================================

/// Artist entity
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Artist {
    pub id: String,
    pub name: String,
    pub genres: Vec<String>,
    pub followers_total: i64,
    pub popularity: i32,
}

/// Album entity
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Album {
    pub id: String,
    pub name: String,
    pub album_type: AlbumType,
    pub label: Option<String>,
    /// Release date as string: "2023-05-15", "2023-05", or "2023"
    pub release_date: Option<String>,
    /// Precision of release_date: "year", "month", or "day"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_date_precision: Option<String>,
    /// Universal Product Code for cross-referencing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_id_upc: Option<String>,
    pub popularity: i32,
}

/// Track entity
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Track {
    pub id: String,
    pub name: String,
    pub album_id: String,
    pub disc_number: i32,
    pub track_number: i32,
    pub duration_ms: i64,
    pub explicit: bool,
    pub popularity: i32,
    /// ISO 639-1 language code or "zxx" for instrumental
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// International Standard Recording Code for cross-referencing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_id_isrc: Option<String>,
}

// =============================================================================
// Image Types (for lazy download from Spotify CDN)
// =============================================================================

/// Image URL from Spotify CDN
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageUrl {
    pub url: String,
    pub width: i32,
    pub height: i32,
}

// =============================================================================
// Relationship Types
// =============================================================================

/// Artist with their role on a track
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrackArtist {
    pub artist: Artist,
    pub role: ArtistRole,
}

/// Disc grouping for album tracks
#[derive(Clone, Debug, Serialize)]
pub struct Disc {
    pub number: i32,
    pub tracks: Vec<Track>,
}

// =============================================================================
// Resolved/Composite Types (API Responses)
// =============================================================================

/// Full artist with all related data
#[derive(Clone, Debug, Serialize)]
pub struct ResolvedArtist {
    pub artist: Artist,
    pub related_artists: Vec<Artist>,
}

/// Full album with tracks and artists
#[derive(Clone, Debug, Serialize)]
pub struct ResolvedAlbum {
    pub album: Album,
    pub artists: Vec<Artist>,
    pub discs: Vec<Disc>,
}

/// Track with its artists and album info
#[derive(Clone, Debug, Serialize)]
pub struct ResolvedTrack {
    pub track: Track,
    pub album: Album,
    pub artists: Vec<TrackArtist>,
}

/// Artist's complete discography
#[derive(Clone, Debug, Serialize)]
pub struct ArtistDiscography {
    pub albums: Vec<Album>,   // Albums where artist is primary
    pub features: Vec<Album>, // Albums where artist appears on
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_artist_role_roundtrip() {
        let roles = vec![
            ArtistRole::MainArtist,
            ArtistRole::FeaturedArtist,
            ArtistRole::Composer,
            ArtistRole::Remixer,
            ArtistRole::Conductor,
            ArtistRole::Orchestra,
        ];
        for role in roles {
            let db_int = role.to_db_int();
            let parsed = ArtistRole::from_db_int(db_int);
            assert_eq!(role, parsed);
        }
    }

    #[test]
    fn test_album_type_roundtrip() {
        let types = vec![
            AlbumType::Album,
            AlbumType::Single,
            AlbumType::Compilation,
            AlbumType::AppearsOn,
        ];
        for album_type in types {
            let db_str = album_type.to_db_str();
            let parsed = AlbumType::from_db_str(db_str);
            assert_eq!(album_type, parsed);
        }
    }

    #[test]
    fn test_album_type_json_serialization() {
        // Verify serde serializes to lowercase
        let album = AlbumType::Album;
        let json = serde_json::to_string(&album).unwrap();
        assert_eq!(json, "\"album\"");

        let single = AlbumType::Single;
        let json = serde_json::to_string(&single).unwrap();
        assert_eq!(json, "\"single\"");

        let appears_on = AlbumType::AppearsOn;
        let json = serde_json::to_string(&appears_on).unwrap();
        assert_eq!(json, "\"appears_on\"");
    }

    #[test]
    fn test_artist_role_json_serialization() {
        // ArtistRole should serialize as string representation
        let role = ArtistRole::MainArtist;
        let json = serde_json::to_string(&role).unwrap();
        // Default derive serialization
        assert!(json.contains("MainArtist"));
    }
}
