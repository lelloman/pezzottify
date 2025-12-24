//! Models for the external downloader service API responses.
//!
//! These types match the JSON structure returned by the downloader service
//! and include conversion methods to catalog models.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// =============================================================================
// Downloader Service Status
// =============================================================================

/// Status response from the downloader service /status endpoint
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DownloaderStatus {
    /// Current state of the downloader
    pub state: String,
    /// Seconds since the downloader started
    pub uptime_secs: u64,
    /// Process ID of the downloader subprocess
    pub downloader_pid: Option<u32>,
    /// Most recent error message
    pub last_error: Option<String>,
}

use crate::catalog_store::{
    ActivityPeriod, Album, AlbumType, Artist, ArtistRole, Format, Image, ImageSize, ImageType,
    Track, TrackAvailability,
};

// =============================================================================
// Downloader API Response Types
// =============================================================================

/// Image from downloader API
#[derive(Clone, Debug, Deserialize)]
pub struct DownloaderImage {
    pub id: String,
    pub size: String,
    pub width: u16,
    pub height: u16,
}

impl DownloaderImage {
    /// Convert to catalog Image model
    pub fn to_catalog_image(&self, uri: String) -> Image {
        Image {
            id: self.id.clone(),
            uri,
            size: ImageSize::from_db_str(&self.size),
            width: self.width,
            height: self.height,
        }
    }

    /// Get the ImageType based on context (portrait for artists, cover for albums)
    pub fn image_type_for_artist() -> ImageType {
        ImageType::PortraitGroup
    }

    pub fn image_type_for_album() -> ImageType {
        ImageType::CoverGroup
    }
}

/// Artist from downloader API
#[derive(Clone, Debug, Deserialize)]
pub struct DownloaderArtist {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub genre: Vec<String>,
    #[serde(default)]
    pub portraits: Vec<DownloaderImage>,
    #[serde(default)]
    pub activity_periods: Vec<DownloaderActivityPeriod>,
    #[serde(default)]
    pub related: Vec<String>,
    #[serde(default)]
    pub portrait_group: Vec<DownloaderImage>,
}

impl DownloaderArtist {
    /// Convert to catalog Artist model
    pub fn to_catalog_artist(&self) -> Artist {
        Artist {
            id: self.id.clone(),
            name: self.name.clone(),
            genres: self.genre.clone(),
            activity_periods: self
                .activity_periods
                .iter()
                .map(|ap| ap.to_catalog_activity_period())
                .collect(),
        }
    }

    /// Get all images (portrait_group preferred, fallback to portraits)
    pub fn get_images(&self) -> &[DownloaderImage] {
        if !self.portrait_group.is_empty() {
            &self.portrait_group
        } else {
            &self.portraits
        }
    }
}

/// Activity period from downloader API
#[derive(Clone, Debug, Deserialize)]
pub struct DownloaderActivityPeriod {
    #[serde(rename = "Decade")]
    pub decade: Option<u16>,
    #[serde(rename = "Timespan")]
    pub timespan: Option<DownloaderTimespan>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DownloaderTimespan {
    pub start_year: u16,
    pub end_year: Option<u16>,
}

impl DownloaderActivityPeriod {
    pub fn to_catalog_activity_period(&self) -> ActivityPeriod {
        if let Some(decade) = self.decade {
            ActivityPeriod::Decade(decade)
        } else if let Some(timespan) = &self.timespan {
            ActivityPeriod::Timespan {
                start_year: timespan.start_year,
                end_year: timespan.end_year,
            }
        } else {
            // Default fallback
            ActivityPeriod::Decade(2000)
        }
    }
}

/// Disc from downloader API
#[derive(Clone, Debug, Deserialize)]
pub struct DownloaderDisc {
    pub number: i32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub tracks: Vec<String>,
}

/// Album from downloader API
#[derive(Clone, Debug, Deserialize)]
pub struct DownloaderAlbum {
    pub id: String,
    pub name: String,
    pub album_type: String,
    #[serde(default)]
    pub artists_ids: Vec<String>,
    pub label: Option<String>,
    pub date: Option<i64>,
    #[serde(default)]
    pub genres: Vec<String>,
    #[serde(default)]
    pub covers: Vec<DownloaderImage>,
    #[serde(default)]
    pub discs: Vec<DownloaderDisc>,
    #[serde(default)]
    pub related: Vec<String>,
    #[serde(default)]
    pub cover_group: Vec<DownloaderImage>,
    pub original_title: Option<String>,
    #[serde(default)]
    pub version_title: String,
}

impl DownloaderAlbum {
    /// Convert to catalog Album model
    pub fn to_catalog_album(&self) -> Album {
        Album {
            id: self.id.clone(),
            name: self.name.clone(),
            album_type: AlbumType::from_db_str(&self.album_type),
            label: self.label.clone(),
            release_date: self.date,
            genres: self.genres.clone(),
            original_title: self.original_title.clone(),
            version_title: if self.version_title.is_empty() {
                None
            } else {
                Some(self.version_title.clone())
            },
        }
    }

    /// Get all images (cover_group preferred, fallback to covers)
    pub fn get_images(&self) -> &[DownloaderImage] {
        if !self.cover_group.is_empty() {
            &self.cover_group
        } else {
            &self.covers
        }
    }

    /// Get all track IDs from all discs
    pub fn get_all_track_ids(&self) -> Vec<String> {
        self.discs.iter().flat_map(|d| d.tracks.clone()).collect()
    }
}

/// Artist with role from downloader API
#[derive(Clone, Debug, Deserialize)]
pub struct DownloaderArtistWithRole {
    pub artist_id: String,
    pub name: String,
    pub role: String,
}

impl DownloaderArtistWithRole {
    /// Convert role string to catalog ArtistRole
    pub fn to_catalog_role(&self) -> ArtistRole {
        match self.role.as_str() {
            "ARTIST_ROLE_MAIN_ARTIST" => ArtistRole::MainArtist,
            "ARTIST_ROLE_FEATURED_ARTIST" => ArtistRole::FeaturedArtist,
            "ARTIST_ROLE_REMIXER" => ArtistRole::Remixer,
            "ARTIST_ROLE_COMPOSER" => ArtistRole::Composer,
            "ARTIST_ROLE_CONDUCTOR" => ArtistRole::Conductor,
            "ARTIST_ROLE_ORCHESTRA" => ArtistRole::Orchestra,
            "ARTIST_ROLE_ACTOR" => ArtistRole::Actor,
            _ => ArtistRole::Unknown,
        }
    }
}

/// Track from downloader API
#[derive(Clone, Debug, Deserialize)]
pub struct DownloaderTrack {
    pub id: String,
    pub name: String,
    pub album_id: String,
    #[serde(default)]
    pub artists_ids: Vec<String>,
    pub number: i32,
    pub disc_number: i32,
    pub duration: i64, // milliseconds
    #[serde(default)]
    pub is_explicit: bool,
    #[serde(default)]
    pub files: HashMap<String, String>,
    #[serde(default)]
    pub alternatives: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub earliest_live_timestamp: Option<i64>,
    #[serde(default)]
    pub has_lyrics: bool,
    #[serde(default)]
    pub language_of_performance: Vec<String>,
    pub original_title: Option<String>,
    #[serde(default)]
    pub version_title: String,
    #[serde(default)]
    pub artists_with_role: Vec<DownloaderArtistWithRole>,
}

impl DownloaderTrack {
    /// Convert to catalog Track model
    pub fn to_catalog_track(&self, audio_uri: String, format: Format) -> Track {
        Track {
            id: self.id.clone(),
            name: self.name.clone(),
            album_id: self.album_id.clone(),
            disc_number: self.disc_number,
            track_number: self.number,
            duration_secs: Some((self.duration / 1000) as i32),
            is_explicit: self.is_explicit,
            audio_uri,
            format,
            tags: self.tags.clone(),
            has_lyrics: self.has_lyrics,
            languages: self.language_of_performance.clone(),
            original_title: self.original_title.clone(),
            version_title: if self.version_title.is_empty() {
                None
            } else {
                Some(self.version_title.clone())
            },
            availability: TrackAvailability::Available,
        }
    }

    /// Get the best available format from files map
    pub fn get_best_format(&self) -> Option<(String, Format)> {
        // Priority order for format selection
        let priority = [
            "OGG_VORBIS_320",
            "OGG_VORBIS_160",
            "OGG_VORBIS_96",
            "MP3_320",
            "MP3_256",
            "MP3_160",
            "MP3_96",
            "AAC_320",
            "AAC_160",
            "AAC_48",
            "AAC_24",
            "FLAC",
        ];

        for format_str in priority {
            if self.files.contains_key(format_str) {
                return Some((format_str.to_string(), Format::from_db_str(format_str)));
            }
        }

        // Fallback to first available
        self.files
            .keys()
            .next()
            .map(|k| (k.clone(), Format::from_db_str(k)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_artist() {
        let json = r#"{
            "id": "5a2EaR3hamoenG9rDuVn8j",
            "name": "Prince",
            "genre": ["funk", "rock"],
            "portraits": [],
            "activity_periods": [{"Decade": 2000}, {"Decade": 1990}],
            "related": ["artist1", "artist2"],
            "portrait_group": [
                {"id": "abc123", "size": "DEFAULT", "width": 320, "height": 320}
            ]
        }"#;

        let artist: DownloaderArtist = serde_json::from_str(json).unwrap();
        assert_eq!(artist.id, "5a2EaR3hamoenG9rDuVn8j");
        assert_eq!(artist.name, "Prince");
        assert_eq!(artist.genre, vec!["funk", "rock"]);
        assert_eq!(artist.related.len(), 2);
        assert_eq!(artist.portrait_group.len(), 1);

        let catalog_artist = artist.to_catalog_artist();
        assert_eq!(catalog_artist.genres, vec!["funk", "rock"]);
    }

    #[test]
    fn test_deserialize_album() {
        let json = r#"{
            "id": "2umoqwMrmjBBPeaqgYu6J9",
            "name": "Purple Rain",
            "album_type": "ALBUM",
            "artists_ids": ["artist1"],
            "label": "Warner Records",
            "date": 456969600,
            "genres": [],
            "covers": [],
            "discs": [{"number": 1, "name": "", "tracks": ["track1", "track2"]}],
            "cover_group": [],
            "original_title": "Purple Rain",
            "version_title": ""
        }"#;

        let album: DownloaderAlbum = serde_json::from_str(json).unwrap();
        assert_eq!(album.id, "2umoqwMrmjBBPeaqgYu6J9");
        assert_eq!(album.name, "Purple Rain");
        assert_eq!(album.get_all_track_ids(), vec!["track1", "track2"]);
    }

    #[test]
    fn test_deserialize_track() {
        let json = r#"{
            "id": "1uvyZBs4IZYRebHIB1747m",
            "name": "Purple Rain",
            "album_id": "album1",
            "artists_ids": ["artist1"],
            "number": 9,
            "disc_number": 1,
            "duration": 521866,
            "is_explicit": false,
            "files": {
                "OGG_VORBIS_160": "hash1",
                "OGG_VORBIS_320": "hash2"
            },
            "tags": [],
            "has_lyrics": true,
            "language_of_performance": ["en"],
            "original_title": "Purple Rain",
            "version_title": "",
            "artists_with_role": [
                {"artist_id": "artist1", "name": "Prince", "role": "ARTIST_ROLE_MAIN_ARTIST"}
            ]
        }"#;

        let track: DownloaderTrack = serde_json::from_str(json).unwrap();
        assert_eq!(track.id, "1uvyZBs4IZYRebHIB1747m");
        assert_eq!(track.duration, 521866);

        let (format_str, format) = track.get_best_format().unwrap();
        assert_eq!(format_str, "OGG_VORBIS_320");
        assert_eq!(format, Format::OggVorbis320);

        let catalog_track = track.to_catalog_track("tracks/test.ogg".to_string(), format);
        assert_eq!(catalog_track.duration_secs, Some(521)); // 521866ms -> 521s
    }

    #[test]
    fn test_artist_role_conversion() {
        let role = DownloaderArtistWithRole {
            artist_id: "test".to_string(),
            name: "Test".to_string(),
            role: "ARTIST_ROLE_MAIN_ARTIST".to_string(),
        };
        assert_eq!(role.to_catalog_role(), ArtistRole::MainArtist);

        let role = DownloaderArtistWithRole {
            artist_id: "test".to_string(),
            name: "Test".to_string(),
            role: "ARTIST_ROLE_FEATURED_ARTIST".to_string(),
        };
        assert_eq!(role.to_catalog_role(), ArtistRole::FeaturedArtist);
    }
}
