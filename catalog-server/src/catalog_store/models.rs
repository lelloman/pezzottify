//! New catalog models for SQLite-backed storage.
//!
//! These models are designed to work with the SQLite database schema
//! and provide cleaner abstractions than the filesystem-based models.

use serde::{Deserialize, Serialize};

// =============================================================================
// Enumerations
// =============================================================================

/// Audio format enumeration
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum Format {
    OggVorbis96,
    OggVorbis160,
    OggVorbis320,
    Mp3_96,
    Mp3_160,
    Mp3_256,
    Mp3_320,
    Aac24,
    Aac48,
    Aac160,
    Aac320,
    Flac,
    Unknown,
}

impl Format {
    /// Convert from database string representation
    pub fn from_db_str(s: &str) -> Self {
        match s {
            "OGG_VORBIS_96" => Format::OggVorbis96,
            "OGG_VORBIS_160" => Format::OggVorbis160,
            "OGG_VORBIS_320" => Format::OggVorbis320,
            "MP3_96" => Format::Mp3_96,
            "MP3_160" => Format::Mp3_160,
            "MP3_256" => Format::Mp3_256,
            "MP3_320" => Format::Mp3_320,
            "AAC_24" => Format::Aac24,
            "AAC_48" => Format::Aac48,
            "AAC_160" => Format::Aac160,
            "AAC_320" => Format::Aac320,
            "FLAC" => Format::Flac,
            _ => Format::Unknown,
        }
    }

    /// Convert to database string representation
    pub fn to_db_str(&self) -> &'static str {
        match self {
            Format::OggVorbis96 => "OGG_VORBIS_96",
            Format::OggVorbis160 => "OGG_VORBIS_160",
            Format::OggVorbis320 => "OGG_VORBIS_320",
            Format::Mp3_96 => "MP3_96",
            Format::Mp3_160 => "MP3_160",
            Format::Mp3_256 => "MP3_256",
            Format::Mp3_320 => "MP3_320",
            Format::Aac24 => "AAC_24",
            Format::Aac48 => "AAC_48",
            Format::Aac160 => "AAC_160",
            Format::Aac320 => "AAC_320",
            Format::Flac => "FLAC",
            Format::Unknown => "UNKNOWN",
        }
    }
}

/// Artist role on a track
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ArtistRole {
    MainArtist,
    FeaturedArtist,
    Remixer,
    Composer,
    Conductor,
    Orchestra,
    Actor,
    Unknown,
}

impl ArtistRole {
    /// Convert from database string representation
    pub fn from_db_str(s: &str) -> Self {
        match s {
            "MAIN_ARTIST" => ArtistRole::MainArtist,
            "FEATURED_ARTIST" => ArtistRole::FeaturedArtist,
            "REMIXER" => ArtistRole::Remixer,
            "COMPOSER" => ArtistRole::Composer,
            "CONDUCTOR" => ArtistRole::Conductor,
            "ORCHESTRA" => ArtistRole::Orchestra,
            "ACTOR" => ArtistRole::Actor,
            _ => ArtistRole::Unknown,
        }
    }

    /// Convert to database string representation
    pub fn to_db_str(&self) -> &'static str {
        match self {
            ArtistRole::MainArtist => "MAIN_ARTIST",
            ArtistRole::FeaturedArtist => "FEATURED_ARTIST",
            ArtistRole::Remixer => "REMIXER",
            ArtistRole::Composer => "COMPOSER",
            ArtistRole::Conductor => "CONDUCTOR",
            ArtistRole::Orchestra => "ORCHESTRA",
            ArtistRole::Actor => "ACTOR",
            ArtistRole::Unknown => "UNKNOWN",
        }
    }
}

/// Activity period for an artist
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ActivityPeriod {
    Timespan {
        start_year: u16,
        end_year: Option<u16>,
    },
    Decade(u16),
}

/// Album type classification
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum AlbumType {
    Album,
    Single,
    Ep,
    Compilation,
    Audiobook,
    Podcast,
}

impl AlbumType {
    /// Convert from database string representation
    pub fn from_db_str(s: &str) -> Self {
        match s {
            "ALBUM" => AlbumType::Album,
            "SINGLE" => AlbumType::Single,
            "EP" => AlbumType::Ep,
            "COMPILATION" => AlbumType::Compilation,
            "AUDIOBOOK" => AlbumType::Audiobook,
            "PODCAST" => AlbumType::Podcast,
            _ => AlbumType::Album, // Default fallback
        }
    }

    /// Convert to database string representation
    pub fn to_db_str(&self) -> &'static str {
        match self {
            AlbumType::Album => "ALBUM",
            AlbumType::Single => "SINGLE",
            AlbumType::Ep => "EP",
            AlbumType::Compilation => "COMPILATION",
            AlbumType::Audiobook => "AUDIOBOOK",
            AlbumType::Podcast => "PODCAST",
        }
    }
}

/// Image size classification
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ImageSize {
    Small,
    Default,
    Large,
    XLarge,
}

impl ImageSize {
    /// Convert from database string representation
    pub fn from_db_str(s: &str) -> Self {
        match s {
            "SMALL" => ImageSize::Small,
            "DEFAULT" => ImageSize::Default,
            "LARGE" => ImageSize::Large,
            "XLARGE" => ImageSize::XLarge,
            _ => ImageSize::Default,
        }
    }

    /// Convert to database string representation
    pub fn to_db_str(&self) -> &'static str {
        match self {
            ImageSize::Small => "SMALL",
            ImageSize::Default => "DEFAULT",
            ImageSize::Large => "LARGE",
            ImageSize::XLarge => "XLARGE",
        }
    }
}

/// Image type for artist/album relationships
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ImageType {
    Portrait,
    PortraitGroup,
    Cover,
    CoverGroup,
}

impl ImageType {
    /// Convert from database string representation
    pub fn from_db_str(s: &str) -> Self {
        match s {
            "portrait" => ImageType::Portrait,
            "portrait_group" => ImageType::PortraitGroup,
            "cover" => ImageType::Cover,
            "cover_group" => ImageType::CoverGroup,
            _ => ImageType::Portrait,
        }
    }

    /// Convert to database string representation
    pub fn to_db_str(&self) -> &'static str {
        match self {
            ImageType::Portrait => "portrait",
            ImageType::PortraitGroup => "portrait_group",
            ImageType::Cover => "cover",
            ImageType::CoverGroup => "cover_group",
        }
    }
}

// =============================================================================
// Core Entities
// =============================================================================

/// Image metadata
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Image {
    pub id: String,
    pub uri: String,
    pub size: ImageSize,
    pub width: u16,
    pub height: u16,
}

/// Artist entity
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Artist {
    pub id: String,
    pub name: String,
    pub genres: Vec<String>,
    pub activity_periods: Vec<ActivityPeriod>,
}

/// Album entity
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Album {
    pub id: String,
    pub name: String,
    pub album_type: AlbumType,
    pub label: Option<String>,
    pub release_date: Option<i64>,
    pub genres: Vec<String>,
    pub original_title: Option<String>,
    pub version_title: Option<String>,
}

/// Track entity
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Track {
    pub id: String,
    pub name: String,
    pub album_id: String,
    pub disc_number: i32,
    pub track_number: i32,
    pub duration_secs: Option<i32>,
    pub is_explicit: bool,
    pub audio_uri: String,
    pub format: Format,
    pub tags: Vec<String>,
    pub has_lyrics: bool,
    pub languages: Vec<String>,
    pub original_title: Option<String>,
    pub version_title: Option<String>,
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
    pub name: Option<String>,
    pub tracks: Vec<Track>,
}

// =============================================================================
// Resolved/Composite Types (API Responses)
// =============================================================================

/// Full artist with all related data
#[derive(Clone, Debug, Serialize)]
pub struct ResolvedArtist {
    pub artist: Artist,
    pub display_image: Option<Image>,
    pub related_artists: Vec<Artist>,
}

/// Full album with tracks and artists
#[derive(Clone, Debug, Serialize)]
pub struct ResolvedAlbum {
    pub album: Album,
    pub artists: Vec<Artist>,
    pub discs: Vec<Disc>,
    pub display_image: Option<Image>,
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
    pub features: Vec<Album>, // Albums where artist is featured on tracks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_roundtrip() {
        let formats = vec![
            Format::OggVorbis96,
            Format::OggVorbis160,
            Format::OggVorbis320,
            Format::Mp3_96,
            Format::Mp3_160,
            Format::Mp3_256,
            Format::Mp3_320,
            Format::Aac24,
            Format::Aac48,
            Format::Aac160,
            Format::Aac320,
            Format::Flac,
            Format::Unknown,
        ];
        for format in formats {
            let db_str = format.to_db_str();
            let parsed = Format::from_db_str(db_str);
            assert_eq!(format, parsed);
        }
    }

    #[test]
    fn test_artist_role_roundtrip() {
        let roles = vec![
            ArtistRole::MainArtist,
            ArtistRole::FeaturedArtist,
            ArtistRole::Remixer,
            ArtistRole::Composer,
            ArtistRole::Conductor,
            ArtistRole::Orchestra,
            ArtistRole::Actor,
            ArtistRole::Unknown,
        ];
        for role in roles {
            let db_str = role.to_db_str();
            let parsed = ArtistRole::from_db_str(db_str);
            assert_eq!(role, parsed);
        }
    }

    #[test]
    fn test_album_type_roundtrip() {
        let types = vec![
            AlbumType::Album,
            AlbumType::Single,
            AlbumType::Ep,
            AlbumType::Compilation,
            AlbumType::Audiobook,
            AlbumType::Podcast,
        ];
        for album_type in types {
            let db_str = album_type.to_db_str();
            let parsed = AlbumType::from_db_str(db_str);
            assert_eq!(album_type, parsed);
        }
    }

    #[test]
    fn test_image_size_roundtrip() {
        let sizes = vec![
            ImageSize::Small,
            ImageSize::Default,
            ImageSize::Large,
            ImageSize::XLarge,
        ];
        for size in sizes {
            let db_str = size.to_db_str();
            let parsed = ImageSize::from_db_str(db_str);
            assert_eq!(size, parsed);
        }
    }

    #[test]
    fn test_image_type_roundtrip() {
        let types = vec![
            ImageType::Portrait,
            ImageType::PortraitGroup,
            ImageType::Cover,
            ImageType::CoverGroup,
        ];
        for image_type in types {
            let db_str = image_type.to_db_str();
            let parsed = ImageType::from_db_str(db_str);
            assert_eq!(image_type, parsed);
        }
    }

    #[test]
    fn test_activity_period_json_serialization() {
        let timespan = ActivityPeriod::Timespan {
            start_year: 1990,
            end_year: Some(2000),
        };
        let json = serde_json::to_string(&timespan).unwrap();
        let parsed: ActivityPeriod = serde_json::from_str(&json).unwrap();
        assert_eq!(timespan, parsed);

        let decade = ActivityPeriod::Decade(1980);
        let json = serde_json::to_string(&decade).unwrap();
        let parsed: ActivityPeriod = serde_json::from_str(&json).unwrap();
        assert_eq!(decade, parsed);
    }
}
