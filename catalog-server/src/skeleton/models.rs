//! Skeleton event models.

use serde::{Deserialize, Serialize};

/// Event types for catalog skeleton changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkeletonEventType {
    ArtistAdded,
    ArtistRemoved,
    AlbumAdded,
    AlbumRemoved,
    TrackAdded,
    TrackRemoved,
}

impl SkeletonEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SkeletonEventType::ArtistAdded => "artist_added",
            SkeletonEventType::ArtistRemoved => "artist_removed",
            SkeletonEventType::AlbumAdded => "album_added",
            SkeletonEventType::AlbumRemoved => "album_removed",
            SkeletonEventType::TrackAdded => "track_added",
            SkeletonEventType::TrackRemoved => "track_removed",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "artist_added" => Some(SkeletonEventType::ArtistAdded),
            "artist_removed" => Some(SkeletonEventType::ArtistRemoved),
            "album_added" => Some(SkeletonEventType::AlbumAdded),
            "album_removed" => Some(SkeletonEventType::AlbumRemoved),
            "track_added" => Some(SkeletonEventType::TrackAdded),
            "track_removed" => Some(SkeletonEventType::TrackRemoved),
            _ => None,
        }
    }
}

/// Payload for album_added events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlbumAddedPayload {
    pub artist_ids: Vec<String>,
}

/// Payload for track_added events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackAddedPayload {
    pub album_id: String,
}

/// A skeleton event stored in the database.
#[derive(Debug, Clone)]
pub struct SkeletonEvent {
    pub seq: i64,
    pub event_type: SkeletonEventType,
    pub entity_id: String,
    pub payload: Option<String>,
    pub timestamp: i64,
}

/// A skeleton change for API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkeletonChange {
    #[serde(rename = "type")]
    pub event_type: String,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist_ids: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album_id: Option<String>,
}

// API response types

/// Response for GET /v1/catalog/skeleton/version
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkeletonVersionResponse {
    pub version: i64,
    pub checksum: String,
}

/// Response for GET /v1/catalog/skeleton
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullSkeletonResponse {
    pub version: i64,
    pub checksum: String,
    pub artists: Vec<String>,
    pub albums: Vec<SkeletonAlbumEntry>,
    pub tracks: Vec<SkeletonTrackEntry>,
}

/// Album entry in skeleton response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkeletonAlbumEntry {
    pub id: String,
    pub artist_ids: Vec<String>,
}

/// Track entry in skeleton response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkeletonTrackEntry {
    pub id: String,
    pub album_id: String,
}

/// Response for GET /v1/catalog/skeleton/delta
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkeletonDeltaResponse {
    pub from_version: i64,
    pub to_version: i64,
    pub checksum: String,
    pub changes: Vec<SkeletonChange>,
}

/// Error response when requested version is too old.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionTooOldError {
    pub error: String,
    pub message: String,
    pub earliest_available: i64,
    pub current_version: i64,
}
