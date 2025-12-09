//! Data models for the download manager.
//!
//! Defines queue items, statuses, priorities, audit logs, and related types.

use serde::{Deserialize, Serialize};

/// Status of a download queue item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QueueStatus {
    Pending,
    InProgress,
    RetryWaiting,
    Completed, // terminal
    Failed,    // terminal
}

impl QueueStatus {
    /// Returns true if this is a terminal state (Completed or Failed).
    pub fn is_terminal(&self) -> bool {
        matches!(self, QueueStatus::Completed | QueueStatus::Failed)
    }
}

/// Priority level for queue items.
/// Lower values = higher priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum QueuePriority {
    Watchdog = 1,  // Highest priority - integrity repairs
    User = 2,      // User requests
    Expansion = 3, // Auto-expansion, discography fills
}

impl QueuePriority {
    pub fn as_i32(&self) -> i32 {
        *self as i32
    }

    pub fn from_i32(value: i32) -> Option<Self> {
        match value {
            1 => Some(QueuePriority::Watchdog),
            2 => Some(QueuePriority::User),
            3 => Some(QueuePriority::Expansion),
            _ => None,
        }
    }
}

/// Type of content being downloaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DownloadContentType {
    Album,       // Full album (metadata + tracks + audio + images)
    TrackAudio,  // Single track audio file
    ArtistImage, // Artist image
    AlbumImage,  // Album cover art
}

impl DownloadContentType {
    pub fn as_str(&self) -> &'static str {
        match self {
            DownloadContentType::Album => "ALBUM",
            DownloadContentType::TrackAudio => "TRACK_AUDIO",
            DownloadContentType::ArtistImage => "ARTIST_IMAGE",
            DownloadContentType::AlbumImage => "ALBUM_IMAGE",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "ALBUM" => Some(DownloadContentType::Album),
            "TRACK_AUDIO" => Some(DownloadContentType::TrackAudio),
            "ARTIST_IMAGE" => Some(DownloadContentType::ArtistImage),
            "ALBUM_IMAGE" => Some(DownloadContentType::AlbumImage),
            _ => None,
        }
    }
}

/// Source of a download request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RequestSource {
    User,      // Explicit user request
    Watchdog,  // Integrity watchdog repair
    Expansion, // Auto-expansion (e.g., related content)
}

impl RequestSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            RequestSource::User => "USER",
            RequestSource::Watchdog => "WATCHDOG",
            RequestSource::Expansion => "EXPANSION",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "USER" => Some(RequestSource::User),
            "WATCHDOG" => Some(RequestSource::Watchdog),
            "EXPANSION" => Some(RequestSource::Expansion),
            _ => None,
        }
    }
}

/// Type of error encountered during download.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadErrorType {
    Connection, // Network error - retry
    Timeout,    // Request timeout - retry
    NotFound,   // Content not found - NO retry (immediate fail)
    Parse,      // Response parse error - retry
    Storage,    // File system error - retry
    Unknown,    // Unknown error - retry
}

impl DownloadErrorType {
    /// Returns true if this error type should trigger a retry.
    pub fn is_retryable(&self) -> bool {
        !matches!(self, DownloadErrorType::NotFound)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            DownloadErrorType::Connection => "connection",
            DownloadErrorType::Timeout => "timeout",
            DownloadErrorType::NotFound => "not_found",
            DownloadErrorType::Parse => "parse",
            DownloadErrorType::Storage => "storage",
            DownloadErrorType::Unknown => "unknown",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "connection" => Some(DownloadErrorType::Connection),
            "timeout" => Some(DownloadErrorType::Timeout),
            "not_found" => Some(DownloadErrorType::NotFound),
            "parse" => Some(DownloadErrorType::Parse),
            "storage" => Some(DownloadErrorType::Storage),
            "unknown" => Some(DownloadErrorType::Unknown),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_queue_status_is_terminal() {
        assert!(!QueueStatus::Pending.is_terminal());
        assert!(!QueueStatus::InProgress.is_terminal());
        assert!(!QueueStatus::RetryWaiting.is_terminal());
        assert!(QueueStatus::Completed.is_terminal());
        assert!(QueueStatus::Failed.is_terminal());
    }

    #[test]
    fn test_queue_priority_ordering() {
        assert!(QueuePriority::Watchdog < QueuePriority::User);
        assert!(QueuePriority::User < QueuePriority::Expansion);
    }

    #[test]
    fn test_queue_priority_conversion() {
        assert_eq!(QueuePriority::Watchdog.as_i32(), 1);
        assert_eq!(QueuePriority::User.as_i32(), 2);
        assert_eq!(QueuePriority::Expansion.as_i32(), 3);

        assert_eq!(QueuePriority::from_i32(1), Some(QueuePriority::Watchdog));
        assert_eq!(QueuePriority::from_i32(2), Some(QueuePriority::User));
        assert_eq!(QueuePriority::from_i32(3), Some(QueuePriority::Expansion));
        assert_eq!(QueuePriority::from_i32(0), None);
        assert_eq!(QueuePriority::from_i32(4), None);
    }

    #[test]
    fn test_download_content_type_conversion() {
        assert_eq!(DownloadContentType::Album.as_str(), "ALBUM");
        assert_eq!(DownloadContentType::TrackAudio.as_str(), "TRACK_AUDIO");
        assert_eq!(DownloadContentType::ArtistImage.as_str(), "ARTIST_IMAGE");
        assert_eq!(DownloadContentType::AlbumImage.as_str(), "ALBUM_IMAGE");

        assert_eq!(
            DownloadContentType::from_str("ALBUM"),
            Some(DownloadContentType::Album)
        );
        assert_eq!(
            DownloadContentType::from_str("TRACK_AUDIO"),
            Some(DownloadContentType::TrackAudio)
        );
        assert_eq!(DownloadContentType::from_str("invalid"), None);
    }

    #[test]
    fn test_request_source_conversion() {
        assert_eq!(RequestSource::User.as_str(), "USER");
        assert_eq!(RequestSource::Watchdog.as_str(), "WATCHDOG");
        assert_eq!(RequestSource::Expansion.as_str(), "EXPANSION");

        assert_eq!(RequestSource::from_str("USER"), Some(RequestSource::User));
        assert_eq!(
            RequestSource::from_str("WATCHDOG"),
            Some(RequestSource::Watchdog)
        );
        assert_eq!(RequestSource::from_str("invalid"), None);
    }

    #[test]
    fn test_download_error_type_retryable() {
        assert!(DownloadErrorType::Connection.is_retryable());
        assert!(DownloadErrorType::Timeout.is_retryable());
        assert!(!DownloadErrorType::NotFound.is_retryable());
        assert!(DownloadErrorType::Parse.is_retryable());
        assert!(DownloadErrorType::Storage.is_retryable());
        assert!(DownloadErrorType::Unknown.is_retryable());
    }

    #[test]
    fn test_download_error_type_conversion() {
        assert_eq!(DownloadErrorType::Connection.as_str(), "connection");
        assert_eq!(DownloadErrorType::NotFound.as_str(), "not_found");

        assert_eq!(
            DownloadErrorType::from_str("connection"),
            Some(DownloadErrorType::Connection)
        );
        assert_eq!(
            DownloadErrorType::from_str("not_found"),
            Some(DownloadErrorType::NotFound)
        );
        assert_eq!(DownloadErrorType::from_str("invalid"), None);
    }

    #[test]
    fn test_queue_status_serialization() {
        let status = QueueStatus::InProgress;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"IN_PROGRESS\"");

        let deserialized: QueueStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, QueueStatus::InProgress);
    }

    #[test]
    fn test_download_content_type_serialization() {
        let content_type = DownloadContentType::TrackAudio;
        let json = serde_json::to_string(&content_type).unwrap();
        assert_eq!(json, "\"TRACK_AUDIO\"");

        let deserialized: DownloadContentType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, DownloadContentType::TrackAudio);
    }

    #[test]
    fn test_download_error_type_serialization() {
        let error_type = DownloadErrorType::NotFound;
        let json = serde_json::to_string(&error_type).unwrap();
        assert_eq!(json, "\"not_found\"");

        let deserialized: DownloadErrorType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, DownloadErrorType::NotFound);
    }
}
