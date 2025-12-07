//! User data models
#![allow(dead_code)] // Models for future API endpoints

use std::collections::HashMap;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

pub struct User {
    pub id: String,
    pub handle: String,
    pub liked_content: HashMap<String, UserLikedContent>,
    pub playlists: HashMap<String, UserPlaylist>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LikedContentType {
    Artist,
    Album,
    Track,
    #[serde(other)]
    Unknown,
}

impl LikedContentType {
    pub fn to_int(&self) -> i32 {
        match self {
            LikedContentType::Artist => 1,
            LikedContentType::Album => 2,
            LikedContentType::Track => 3,
            LikedContentType::Unknown => 0,
        }
    }

    pub fn from_int(value: i32) -> Self {
        match value {
            1 => LikedContentType::Artist,
            2 => LikedContentType::Album,
            3 => LikedContentType::Track,
            _ => LikedContentType::Unknown,
        }
    }
}

pub struct UserLikedContent {
    pub timestamp: SystemTime,
    pub content_id: String,
    pub content_type: LikedContentType,
}

#[derive(Serialize, Debug)]
pub struct UserPlaylist {
    pub id: String,
    pub user_id: usize,
    pub creator: String,
    pub name: String,
    pub created: SystemTime,
    pub tracks: Vec<String>,
}

/// Bandwidth usage record for a specific user, date, and endpoint category
#[derive(Serialize, Debug, Clone)]
pub struct BandwidthUsage {
    pub user_id: usize,
    /// Date in YYYYMMDD format
    pub date: u32,
    pub endpoint_category: String,
    pub bytes_sent: u64,
    pub request_count: u64,
}

/// Summary of bandwidth usage across multiple records
#[derive(Serialize, Debug, Clone)]
pub struct BandwidthSummary {
    pub user_id: Option<usize>,
    pub total_bytes_sent: u64,
    pub total_requests: u64,
    /// Breakdown by endpoint category
    pub by_category: HashMap<String, CategoryBandwidth>,
}

/// Bandwidth stats for a specific category
#[derive(Serialize, Debug, Clone)]
pub struct CategoryBandwidth {
    pub bytes_sent: u64,
    pub request_count: u64,
}

// ============================================================================
// Listening Stats Models
// ============================================================================

/// Individual listening event recorded when a user plays a track
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ListeningEvent {
    pub id: Option<usize>,
    pub user_id: usize,
    pub track_id: String,
    /// Client-generated UUID for deduplication (supports offline queue retry)
    pub session_id: Option<String>,
    /// Unix timestamp when playback started
    pub started_at: u64,
    /// Unix timestamp when playback ended
    pub ended_at: Option<u64>,
    /// Actual listening time in seconds (excluding pauses)
    pub duration_seconds: u32,
    /// Total track duration in seconds (for completion calculation)
    pub track_duration_seconds: u32,
    /// True if >90% of track was played
    pub completed: bool,
    /// Number of seek operations during playback
    pub seek_count: u32,
    /// Number of pause/resume cycles
    pub pause_count: u32,
    /// Context: "album", "playlist", "track", "search"
    pub playback_context: Option<String>,
    /// Client type: "web", "android", "ios"
    pub client_type: Option<String>,
    /// Date in YYYYMMDD format for efficient queries
    pub date: u32,
}

/// Summary of listening activity for a user or platform
#[derive(Serialize, Debug, Clone)]
pub struct ListeningSummary {
    pub user_id: Option<usize>,
    pub total_plays: u64,
    pub total_duration_seconds: u64,
    pub completed_plays: u64,
    pub unique_tracks: u64,
}

/// Per-track listening statistics
#[derive(Serialize, Debug, Clone)]
pub struct TrackListeningStats {
    pub track_id: String,
    pub play_count: u64,
    pub total_duration_seconds: u64,
    pub completed_count: u64,
    pub unique_listeners: u64,
}

/// Entry in a user's listening history
#[derive(Serialize, Debug, Clone)]
pub struct UserListeningHistoryEntry {
    pub track_id: String,
    pub last_played_at: u64,
    pub play_count: u64,
    pub total_duration_seconds: u64,
}

/// Daily aggregated listening stats (for admin analytics)
#[derive(Serialize, Debug, Clone)]
pub struct DailyListeningStats {
    /// Date in YYYYMMDD format
    pub date: u32,
    pub total_plays: u64,
    pub total_duration_seconds: u64,
    pub completed_plays: u64,
    pub unique_users: u64,
    pub unique_tracks: u64,
}

// ============================================================================
// Popular Content Models
// ============================================================================

/// A popular album with listening statistics
#[derive(Serialize, Debug, Clone)]
pub struct PopularAlbum {
    pub id: String,
    pub name: String,
    /// Image ID for the album cover
    pub image_id: Option<String>,
    /// Primary artist names for display
    pub artist_names: Vec<String>,
    /// Total play count across all tracks in the album
    pub play_count: u64,
}

/// A popular artist with listening statistics
#[derive(Serialize, Debug, Clone)]
pub struct PopularArtist {
    pub id: String,
    pub name: String,
    /// Image ID for the artist image
    pub image_id: Option<String>,
    /// Total play count across all tracks by the artist
    pub play_count: u64,
}

/// Container for popular content (albums and artists)
#[derive(Serialize, Debug, Clone)]
pub struct PopularContent {
    pub albums: Vec<PopularAlbum>,
    pub artists: Vec<PopularArtist>,
}
