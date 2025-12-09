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

/// A download queue item representing a single download task.
#[derive(Debug, Clone)]
pub struct QueueItem {
    /// Unique identifier (UUID)
    pub id: String,
    /// Parent queue item ID (for child items like individual tracks of an album)
    pub parent_id: Option<String>,
    /// Current status in the state machine
    pub status: QueueStatus,
    /// Processing priority (lower value = higher priority)
    pub priority: QueuePriority,
    /// Type of content being downloaded
    pub content_type: DownloadContentType,
    /// External ID from music provider (base62 for content, hex for images)
    pub content_id: String,
    /// Display name (album/artist name)
    pub content_name: Option<String>,
    /// Artist name for display
    pub artist_name: Option<String>,
    /// Source of this request
    pub request_source: RequestSource,
    /// User who requested this download (if user-initiated)
    pub requested_by_user_id: Option<String>,
    /// When the item was added to the queue (Unix timestamp)
    pub created_at: i64,
    /// When processing started (IN_PROGRESS state began)
    pub started_at: Option<i64>,
    /// When the item reached a terminal state
    pub completed_at: Option<i64>,
    /// Timestamp of the last download attempt
    pub last_attempt_at: Option<i64>,
    /// When to retry (for RETRY_WAITING status)
    pub next_retry_at: Option<i64>,
    /// Number of retry attempts made
    pub retry_count: i32,
    /// Maximum number of retries allowed
    pub max_retries: i32,
    /// Type of error that caused failure (if any)
    pub error_type: Option<DownloadErrorType>,
    /// Human-readable error message
    pub error_message: Option<String>,
    /// Number of bytes downloaded (for completed items)
    pub bytes_downloaded: Option<u64>,
    /// Time spent processing in milliseconds
    pub processing_duration_ms: Option<i64>,
}

impl QueueItem {
    /// Create a new queue item with the given parameters.
    pub fn new(
        id: String,
        content_type: DownloadContentType,
        content_id: String,
        priority: QueuePriority,
        request_source: RequestSource,
        max_retries: i32,
    ) -> Self {
        Self {
            id,
            parent_id: None,
            status: QueueStatus::Pending,
            priority,
            content_type,
            content_id,
            content_name: None,
            artist_name: None,
            request_source,
            requested_by_user_id: None,
            created_at: chrono::Utc::now().timestamp(),
            started_at: None,
            completed_at: None,
            last_attempt_at: None,
            next_retry_at: None,
            retry_count: 0,
            max_retries,
            error_type: None,
            error_message: None,
            bytes_downloaded: None,
            processing_duration_ms: None,
        }
    }

    /// Create a new queue item as a child of another item.
    pub fn new_child(
        id: String,
        parent_id: String,
        content_type: DownloadContentType,
        content_id: String,
        priority: QueuePriority,
        request_source: RequestSource,
        requested_by_user_id: Option<String>,
        max_retries: i32,
    ) -> Self {
        Self {
            id,
            parent_id: Some(parent_id),
            status: QueueStatus::Pending,
            priority,
            content_type,
            content_id,
            content_name: None,
            artist_name: None,
            request_source,
            requested_by_user_id,
            created_at: chrono::Utc::now().timestamp(),
            started_at: None,
            completed_at: None,
            last_attempt_at: None,
            next_retry_at: None,
            retry_count: 0,
            max_retries,
            error_type: None,
            error_message: None,
            bytes_downloaded: None,
            processing_duration_ms: None,
        }
    }

    /// Set display names for this item.
    pub fn with_names(mut self, content_name: Option<String>, artist_name: Option<String>) -> Self {
        self.content_name = content_name;
        self.artist_name = artist_name;
        self
    }

    /// Set the user who requested this download.
    pub fn with_user(mut self, user_id: String) -> Self {
        self.requested_by_user_id = Some(user_id);
        self
    }

    /// Returns true if this is a parent item (has no parent_id).
    pub fn is_parent(&self) -> bool {
        self.parent_id.is_none()
    }

    /// Returns true if this is a child item.
    pub fn is_child(&self) -> bool {
        self.parent_id.is_some()
    }
}

/// Progress information for a download with children (e.g., album with tracks).
#[derive(Debug, Clone, Serialize, Default)]
pub struct DownloadProgress {
    /// Total number of child items
    pub total_children: usize,
    /// Number of completed children
    pub completed: usize,
    /// Number of failed children
    pub failed: usize,
    /// Number of pending children
    pub pending: usize,
    /// Number of in-progress children
    pub in_progress: usize,
}

impl DownloadProgress {
    /// Returns the percentage of completion (0-100).
    pub fn percentage(&self) -> u8 {
        if self.total_children == 0 {
            return 0;
        }
        let terminal = self.completed + self.failed;
        ((terminal * 100) / self.total_children) as u8
    }

    /// Returns true if all children have reached a terminal state.
    pub fn is_complete(&self) -> bool {
        self.total_children > 0 && (self.completed + self.failed) == self.total_children
    }
}

/// Simplified view of a queue item for user-facing API responses.
#[derive(Debug, Clone, Serialize)]
pub struct UserRequestView {
    /// Queue item ID
    pub id: String,
    /// Type of content being downloaded
    pub content_type: DownloadContentType,
    /// External content ID
    pub content_id: String,
    /// Display name (album/artist name)
    pub content_name: String,
    /// Artist name for display
    pub artist_name: Option<String>,
    /// Current status
    pub status: QueueStatus,
    /// When the request was created (Unix timestamp)
    pub created_at: i64,
    /// When the request completed (Unix timestamp)
    pub completed_at: Option<i64>,
    /// Error message if failed
    pub error_message: Option<String>,
    /// Progress for album requests (shows child item status)
    pub progress: Option<DownloadProgress>,
    /// Position in queue (for pending items)
    pub queue_position: Option<usize>,
}

impl UserRequestView {
    /// Create a UserRequestView from a QueueItem.
    pub fn from_queue_item(item: &QueueItem, progress: Option<DownloadProgress>, queue_position: Option<usize>) -> Self {
        Self {
            id: item.id.clone(),
            content_type: item.content_type,
            content_id: item.content_id.clone(),
            content_name: item.content_name.clone().unwrap_or_else(|| item.content_id.clone()),
            artist_name: item.artist_name.clone(),
            status: item.status,
            created_at: item.created_at,
            completed_at: item.completed_at,
            error_message: item.error_message.clone(),
            progress,
            queue_position,
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

    #[test]
    fn test_queue_item_new() {
        let item = QueueItem::new(
            "test-id".to_string(),
            DownloadContentType::Album,
            "album-123".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );

        assert_eq!(item.id, "test-id");
        assert!(item.parent_id.is_none());
        assert_eq!(item.status, QueueStatus::Pending);
        assert_eq!(item.priority, QueuePriority::User);
        assert_eq!(item.content_type, DownloadContentType::Album);
        assert_eq!(item.content_id, "album-123");
        assert_eq!(item.request_source, RequestSource::User);
        assert_eq!(item.max_retries, 5);
        assert_eq!(item.retry_count, 0);
        assert!(item.is_parent());
        assert!(!item.is_child());
    }

    #[test]
    fn test_queue_item_new_child() {
        let item = QueueItem::new_child(
            "child-id".to_string(),
            "parent-id".to_string(),
            DownloadContentType::TrackAudio,
            "track-456".to_string(),
            QueuePriority::User,
            RequestSource::User,
            Some("user-1".to_string()),
            3,
        );

        assert_eq!(item.id, "child-id");
        assert_eq!(item.parent_id, Some("parent-id".to_string()));
        assert_eq!(item.status, QueueStatus::Pending);
        assert_eq!(item.content_type, DownloadContentType::TrackAudio);
        assert_eq!(item.requested_by_user_id, Some("user-1".to_string()));
        assert!(!item.is_parent());
        assert!(item.is_child());
    }

    #[test]
    fn test_queue_item_with_names() {
        let item = QueueItem::new(
            "test-id".to_string(),
            DownloadContentType::Album,
            "album-123".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        )
        .with_names(Some("Album Name".to_string()), Some("Artist Name".to_string()));

        assert_eq!(item.content_name, Some("Album Name".to_string()));
        assert_eq!(item.artist_name, Some("Artist Name".to_string()));
    }

    #[test]
    fn test_queue_item_with_user() {
        let item = QueueItem::new(
            "test-id".to_string(),
            DownloadContentType::Album,
            "album-123".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        )
        .with_user("user-123".to_string());

        assert_eq!(item.requested_by_user_id, Some("user-123".to_string()));
    }

    #[test]
    fn test_download_progress_percentage() {
        let progress = DownloadProgress {
            total_children: 10,
            completed: 5,
            failed: 2,
            pending: 2,
            in_progress: 1,
        };
        assert_eq!(progress.percentage(), 70); // (5+2)/10 * 100 = 70%

        let empty = DownloadProgress::default();
        assert_eq!(empty.percentage(), 0);
    }

    #[test]
    fn test_download_progress_is_complete() {
        let incomplete = DownloadProgress {
            total_children: 10,
            completed: 5,
            failed: 2,
            pending: 2,
            in_progress: 1,
        };
        assert!(!incomplete.is_complete());

        let complete = DownloadProgress {
            total_children: 10,
            completed: 8,
            failed: 2,
            pending: 0,
            in_progress: 0,
        };
        assert!(complete.is_complete());

        let empty = DownloadProgress::default();
        assert!(!empty.is_complete());
    }

    #[test]
    fn test_user_request_view_from_queue_item() {
        let item = QueueItem::new(
            "test-id".to_string(),
            DownloadContentType::Album,
            "album-123".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        )
        .with_names(Some("Test Album".to_string()), Some("Test Artist".to_string()));

        let progress = DownloadProgress {
            total_children: 10,
            completed: 5,
            failed: 0,
            pending: 3,
            in_progress: 2,
        };

        let view = UserRequestView::from_queue_item(&item, Some(progress.clone()), Some(3));

        assert_eq!(view.id, "test-id");
        assert_eq!(view.content_type, DownloadContentType::Album);
        assert_eq!(view.content_id, "album-123");
        assert_eq!(view.content_name, "Test Album");
        assert_eq!(view.artist_name, Some("Test Artist".to_string()));
        assert_eq!(view.status, QueueStatus::Pending);
        assert!(view.progress.is_some());
        assert_eq!(view.queue_position, Some(3));
    }

    #[test]
    fn test_user_request_view_fallback_content_name() {
        let item = QueueItem::new(
            "test-id".to_string(),
            DownloadContentType::Album,
            "album-123".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        // No content_name set, should fall back to content_id

        let view = UserRequestView::from_queue_item(&item, None, None);

        assert_eq!(view.content_name, "album-123"); // Falls back to content_id
    }

    #[test]
    fn test_download_progress_serialization() {
        let progress = DownloadProgress {
            total_children: 10,
            completed: 5,
            failed: 2,
            pending: 2,
            in_progress: 1,
        };

        let json = serde_json::to_string(&progress).unwrap();
        assert!(json.contains("\"total_children\":10"));
        assert!(json.contains("\"completed\":5"));
    }
}
