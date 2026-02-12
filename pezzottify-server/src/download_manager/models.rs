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

    /// Convert to database string representation.
    pub fn as_db_str(&self) -> &'static str {
        match self {
            QueueStatus::Pending => "PENDING",
            QueueStatus::InProgress => "IN_PROGRESS",
            QueueStatus::RetryWaiting => "RETRY_WAITING",
            QueueStatus::Completed => "COMPLETED",
            QueueStatus::Failed => "FAILED",
        }
    }

    /// Parse from database string representation.
    pub fn from_db_str(s: &str) -> Self {
        match s {
            "PENDING" => QueueStatus::Pending,
            "IN_PROGRESS" => QueueStatus::InProgress,
            "RETRY_WAITING" => QueueStatus::RetryWaiting,
            "COMPLETED" => QueueStatus::Completed,
            "FAILED" => QueueStatus::Failed,
            _ => QueueStatus::Pending, // Default fallback
        }
    }
}

/// Priority level for queue items.
/// Lower values = higher priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum QueuePriority {
    Urgent = 1,     // Highest priority - critical repairs
    User = 2,       // User requests
    Expansion = 3,  // Auto-expansion, discography fills
    Background = 4, // Lowest priority - background enrichment (integrity watchdog)
}

impl QueuePriority {
    pub fn as_i32(&self) -> i32 {
        *self as i32
    }

    pub fn from_i32(value: i32) -> Option<Self> {
        match value {
            1 => Some(QueuePriority::Urgent),
            2 => Some(QueuePriority::User),
            3 => Some(QueuePriority::Expansion),
            4 => Some(QueuePriority::Background),
            _ => None,
        }
    }
}

/// Type of content being downloaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DownloadContentType {
    Album,          // Full album (metadata + tracks + audio + images)
    TrackAudio,     // Single track audio file
    ArtistImage,    // Artist image
    AlbumImage,     // Album cover art
    ArtistRelated,  // Fetch related artist IDs for an artist
    ArtistMetadata, // Fetch full artist metadata and create record
}

impl DownloadContentType {
    pub fn as_str(&self) -> &'static str {
        match self {
            DownloadContentType::Album => "ALBUM",
            DownloadContentType::TrackAudio => "TRACK_AUDIO",
            DownloadContentType::ArtistImage => "ARTIST_IMAGE",
            DownloadContentType::AlbumImage => "ALBUM_IMAGE",
            DownloadContentType::ArtistRelated => "ARTIST_RELATED",
            DownloadContentType::ArtistMetadata => "ARTIST_METADATA",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "ALBUM" => Some(DownloadContentType::Album),
            "TRACK_AUDIO" => Some(DownloadContentType::TrackAudio),
            "ARTIST_IMAGE" => Some(DownloadContentType::ArtistImage),
            "ALBUM_IMAGE" => Some(DownloadContentType::AlbumImage),
            "ARTIST_RELATED" => Some(DownloadContentType::ArtistRelated),
            "ARTIST_METADATA" => Some(DownloadContentType::ArtistMetadata),
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

    #[allow(clippy::should_implement_trait)]
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
    Corruption, // File validation failed (ffprobe) - retry, triggers corruption handler
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
            DownloadErrorType::Corruption => "corruption",
            DownloadErrorType::Unknown => "unknown",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "connection" => Some(DownloadErrorType::Connection),
            "timeout" => Some(DownloadErrorType::Timeout),
            "not_found" => Some(DownloadErrorType::NotFound),
            "parse" => Some(DownloadErrorType::Parse),
            "storage" => Some(DownloadErrorType::Storage),
            "corruption" => Some(DownloadErrorType::Corruption),
            "unknown" => Some(DownloadErrorType::Unknown),
            _ => None,
        }
    }

    /// Returns true if this error type indicates file corruption (ffprobe failure).
    pub fn is_corruption(&self) -> bool {
        matches!(self, DownloadErrorType::Corruption)
    }
}

/// A download queue item representing a single download task.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    #[allow(clippy::too_many_arguments)]
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
    pub fn from_queue_item(
        item: &QueueItem,
        progress: Option<DownloadProgress>,
        queue_position: Option<usize>,
    ) -> Self {
        Self {
            id: item.id.clone(),
            content_type: item.content_type,
            content_id: item.content_id.clone(),
            content_name: item
                .content_name
                .clone()
                .unwrap_or_else(|| item.content_id.clone()),
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

/// Types of events recorded in the audit log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuditEventType {
    /// User submitted an album/content request
    RequestCreated,
    /// Item claimed for processing (status -> IN_PROGRESS)
    DownloadStarted,
    /// Album spawned child items (track audio, images)
    ChildrenCreated,
    /// Item finished successfully
    DownloadCompleted,
    /// Item failed after max retries
    DownloadFailed,
    /// Item scheduled for retry (status -> RETRY_WAITING)
    RetryScheduled,
    /// Admin manually reset failed item to pending
    AdminRetry,
    /// Watchdog queued a repair item
    WatchdogQueued,
    /// Watchdog scan started
    WatchdogScanStarted,
    /// Watchdog scan completed
    WatchdogScanCompleted,
}

impl AuditEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AuditEventType::RequestCreated => "REQUEST_CREATED",
            AuditEventType::DownloadStarted => "DOWNLOAD_STARTED",
            AuditEventType::ChildrenCreated => "CHILDREN_CREATED",
            AuditEventType::DownloadCompleted => "DOWNLOAD_COMPLETED",
            AuditEventType::DownloadFailed => "DOWNLOAD_FAILED",
            AuditEventType::RetryScheduled => "RETRY_SCHEDULED",
            AuditEventType::AdminRetry => "ADMIN_RETRY",
            AuditEventType::WatchdogQueued => "WATCHDOG_QUEUED",
            AuditEventType::WatchdogScanStarted => "WATCHDOG_SCAN_STARTED",
            AuditEventType::WatchdogScanCompleted => "WATCHDOG_SCAN_COMPLETED",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "REQUEST_CREATED" => Some(AuditEventType::RequestCreated),
            "DOWNLOAD_STARTED" => Some(AuditEventType::DownloadStarted),
            "CHILDREN_CREATED" => Some(AuditEventType::ChildrenCreated),
            "DOWNLOAD_COMPLETED" => Some(AuditEventType::DownloadCompleted),
            "DOWNLOAD_FAILED" => Some(AuditEventType::DownloadFailed),
            "RETRY_SCHEDULED" => Some(AuditEventType::RetryScheduled),
            "ADMIN_RETRY" => Some(AuditEventType::AdminRetry),
            "WATCHDOG_QUEUED" => Some(AuditEventType::WatchdogQueued),
            "WATCHDOG_SCAN_STARTED" => Some(AuditEventType::WatchdogScanStarted),
            "WATCHDOG_SCAN_COMPLETED" => Some(AuditEventType::WatchdogScanCompleted),
            _ => None,
        }
    }
}

/// An entry in the audit log.
#[derive(Debug, Clone, Serialize)]
pub struct AuditLogEntry {
    /// Auto-incremented ID
    pub id: i64,
    /// Unix timestamp when the event occurred
    pub timestamp: i64,
    /// Type of event
    pub event_type: AuditEventType,
    /// Associated queue item ID (if applicable)
    pub queue_item_id: Option<String>,
    /// Content type (if applicable)
    pub content_type: Option<DownloadContentType>,
    /// Content ID (if applicable)
    pub content_id: Option<String>,
    /// User who triggered the event (if applicable)
    pub user_id: Option<String>,
    /// Source of the request
    pub request_source: Option<RequestSource>,
    /// Event-specific JSON data (e.g., child count for ChildrenCreated)
    pub details: Option<serde_json::Value>,
}

impl AuditLogEntry {
    /// Create a new audit log entry with the current timestamp.
    pub fn new(event_type: AuditEventType) -> Self {
        Self {
            id: 0, // Will be set by database
            timestamp: chrono::Utc::now().timestamp(),
            event_type,
            queue_item_id: None,
            content_type: None,
            content_id: None,
            user_id: None,
            request_source: None,
            details: None,
        }
    }

    /// Set the queue item ID.
    pub fn with_queue_item(mut self, queue_item_id: String) -> Self {
        self.queue_item_id = Some(queue_item_id);
        self
    }

    /// Set content information.
    pub fn with_content(mut self, content_type: DownloadContentType, content_id: String) -> Self {
        self.content_type = Some(content_type);
        self.content_id = Some(content_id);
        self
    }

    /// Set user ID.
    pub fn with_user(mut self, user_id: String) -> Self {
        self.user_id = Some(user_id);
        self
    }

    /// Set request source.
    pub fn with_source(mut self, source: RequestSource) -> Self {
        self.request_source = Some(source);
        self
    }

    /// Set event details as JSON.
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
}

/// Filter criteria for querying audit logs.
#[derive(Debug, Clone, Default)]
pub struct AuditLogFilter {
    /// Filter by queue item ID
    pub queue_item_id: Option<String>,
    /// Filter by user ID
    pub user_id: Option<String>,
    /// Filter by event types (any of these)
    pub event_types: Option<Vec<AuditEventType>>,
    /// Filter by content type
    pub content_type: Option<DownloadContentType>,
    /// Filter by content ID
    pub content_id: Option<String>,
    /// Filter events since this timestamp
    pub since: Option<i64>,
    /// Filter events until this timestamp
    pub until: Option<i64>,
    /// Maximum number of results
    pub limit: usize,
    /// Offset for pagination
    pub offset: usize,
}

impl AuditLogFilter {
    /// Create a new filter with default limit.
    pub fn new() -> Self {
        Self {
            limit: 100,
            ..Default::default()
        }
    }

    /// Filter by queue item.
    pub fn for_queue_item(mut self, queue_item_id: String) -> Self {
        self.queue_item_id = Some(queue_item_id);
        self
    }

    /// Filter by user.
    pub fn for_user(mut self, user_id: String) -> Self {
        self.user_id = Some(user_id);
        self
    }

    /// Filter by event types.
    pub fn with_event_types(mut self, types: Vec<AuditEventType>) -> Self {
        self.event_types = Some(types);
        self
    }

    /// Filter by content type.
    pub fn for_content_type(mut self, content_type: DownloadContentType) -> Self {
        self.content_type = Some(content_type);
        self
    }

    /// Filter by content ID.
    pub fn for_content_id(mut self, content_id: String) -> Self {
        self.content_id = Some(content_id);
        self
    }

    /// Filter by time range.
    pub fn in_range(mut self, since: Option<i64>, until: Option<i64>) -> Self {
        self.since = since;
        self.until = until;
        self
    }

    /// Set pagination.
    pub fn paginate(mut self, limit: usize, offset: usize) -> Self {
        self.limit = limit;
        self.offset = offset;
        self
    }
}

/// User's rate limit status for download requests.
#[derive(Debug, Clone, Serialize)]
pub struct UserLimitStatus {
    /// Number of requests made today
    pub requests_today: i32,
    /// Maximum requests allowed per day
    pub max_per_day: i32,
    /// Number of items currently in queue for this user
    pub in_queue: i32,
    /// Maximum items allowed in queue
    pub max_queue: i32,
    /// Whether the user can make more requests
    pub can_request: bool,
}

impl UserLimitStatus {
    /// Create a new status indicating the user can make requests.
    pub fn available(requests_today: i32, max_per_day: i32, in_queue: i32, max_queue: i32) -> Self {
        Self {
            requests_today,
            max_per_day,
            in_queue,
            max_queue,
            can_request: requests_today < max_per_day && in_queue < max_queue,
        }
    }
}

/// System-wide capacity status for download rate limiting.
#[derive(Debug, Clone, Serialize)]
pub struct CapacityStatus {
    /// Albums downloaded this hour
    pub albums_this_hour: i32,
    /// Maximum albums per hour
    pub max_per_hour: i32,
    /// Albums downloaded today
    pub albums_today: i32,
    /// Maximum albums per day
    pub max_per_day: i32,
    /// Whether the system is at capacity
    pub at_capacity: bool,
}

impl CapacityStatus {
    /// Create a new capacity status.
    pub fn new(
        albums_this_hour: i32,
        max_per_hour: i32,
        albums_today: i32,
        max_per_day: i32,
    ) -> Self {
        Self {
            albums_this_hour,
            max_per_hour,
            albums_today,
            max_per_day,
            at_capacity: albums_this_hour >= max_per_hour || albums_today >= max_per_day,
        }
    }
}

/// Queue statistics summary.
#[derive(Debug, Clone, Serialize, Default)]
pub struct QueueStats {
    /// Items waiting to be processed
    pub pending: i64,
    /// Items currently being processed
    pub in_progress: i64,
    /// Items waiting to retry
    pub retry_waiting: i64,
    /// Items completed today
    pub completed_today: i64,
    /// Items failed today
    pub failed_today: i64,
}

/// Activity log entry for tracking download throughput.
#[derive(Debug, Clone)]
pub struct ActivityLogEntry {
    /// Unix timestamp truncated to hour
    pub hour_bucket: i64,
    /// Number of albums downloaded in this hour
    pub albums_downloaded: i64,
    /// Number of tracks downloaded in this hour
    pub tracks_downloaded: i64,
    /// Number of images downloaded in this hour
    pub images_downloaded: i64,
    /// Total bytes downloaded in this hour
    pub bytes_downloaded: i64,
    /// Number of failed downloads in this hour
    pub failed_count: i64,
}

/// Download counts for the current hour.
#[derive(Debug, Clone, Serialize, Default)]
pub struct HourlyCounts {
    pub albums: i64,
    pub tracks: i64,
    pub images: i64,
    pub bytes: i64,
}

/// Download counts for the current day.
#[derive(Debug, Clone, Serialize, Default)]
pub struct DailyCounts {
    pub albums: i64,
    pub tracks: i64,
    pub images: i64,
    pub bytes: i64,
}

/// Time period for aggregating download statistics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StatsPeriod {
    /// Hourly granularity (last 48 hours)
    Hourly,
    /// Daily granularity (last 30 days)
    Daily,
    /// Weekly granularity (last 12 weeks)
    Weekly,
}

impl StatsPeriod {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "hourly" => Some(StatsPeriod::Hourly),
            "daily" => Some(StatsPeriod::Daily),
            "weekly" => Some(StatsPeriod::Weekly),
            _ => None,
        }
    }
}

/// A single entry in aggregated download statistics.
#[derive(Debug, Clone, Serialize)]
pub struct StatsHistoryEntry {
    /// Unix timestamp of the period start (hour/day/week boundary)
    pub period_start: i64,
    /// Number of albums downloaded in this period
    pub albums: i64,
    /// Number of tracks downloaded in this period
    pub tracks: i64,
    /// Number of images downloaded in this period
    pub images: i64,
    /// Total bytes downloaded in this period
    pub bytes: i64,
    /// Number of failed downloads in this period
    pub failures: i64,
}

/// Aggregated download statistics over time.
#[derive(Debug, Clone, Serialize)]
pub struct DownloadStatsHistory {
    /// The aggregation period used
    pub period: StatsPeriod,
    /// Time-series entries (oldest first)
    pub entries: Vec<StatsHistoryEntry>,
    /// Total albums downloaded across all entries
    pub total_albums: i64,
    /// Total tracks downloaded across all entries
    pub total_tracks: i64,
    /// Total images downloaded across all entries
    pub total_images: i64,
    /// Total bytes downloaded across all entries
    pub total_bytes: i64,
    /// Total failures across all entries
    pub total_failures: i64,
}

impl DownloadStatsHistory {
    /// Create a new stats history with computed totals.
    pub fn new(period: StatsPeriod, entries: Vec<StatsHistoryEntry>) -> Self {
        let (total_albums, total_tracks, total_images, total_bytes, total_failures) =
            entries.iter().fold((0, 0, 0, 0, 0), |acc, e| {
                (
                    acc.0 + e.albums,
                    acc.1 + e.tracks,
                    acc.2 + e.images,
                    acc.3 + e.bytes,
                    acc.4 + e.failures,
                )
            });

        Self {
            period,
            entries,
            total_albums,
            total_tracks,
            total_images,
            total_bytes,
            total_failures,
        }
    }
}

/// Error information for failed downloads.
#[derive(Debug, Clone)]
pub struct DownloadError {
    /// Type of error
    pub error_type: DownloadErrorType,
    /// Human-readable error message
    pub message: String,
}

impl DownloadError {
    /// Create a new download error.
    pub fn new(error_type: DownloadErrorType, message: impl Into<String>) -> Self {
        Self {
            error_type,
            message: message.into(),
        }
    }

    /// Returns true if this error should trigger a retry.
    pub fn is_retryable(&self) -> bool {
        self.error_type.is_retryable()
    }
}

/// Result of processing a queue item.
#[derive(Debug, Clone)]
pub struct ProcessingResult {
    /// ID of the processed queue item
    pub queue_item_id: String,
    /// Type of content that was processed
    pub content_type: DownloadContentType,
    /// Whether the processing succeeded
    pub success: bool,
    /// Number of bytes downloaded (if successful)
    pub bytes_downloaded: Option<u64>,
    /// Time spent processing in milliseconds
    pub duration_ms: i64,
    /// Error information (if failed)
    pub error: Option<DownloadError>,
}

impl ProcessingResult {
    /// Create a successful result.
    pub fn success(
        queue_item_id: String,
        content_type: DownloadContentType,
        bytes_downloaded: u64,
        duration_ms: i64,
    ) -> Self {
        Self {
            queue_item_id,
            content_type,
            success: true,
            bytes_downloaded: Some(bytes_downloaded),
            duration_ms,
            error: None,
        }
    }

    /// Create a failed result.
    pub fn failure(
        queue_item_id: String,
        content_type: DownloadContentType,
        duration_ms: i64,
        error: DownloadError,
    ) -> Self {
        Self {
            queue_item_id,
            content_type,
            success: false,
            bytes_downloaded: None,
            duration_ms,
            error: Some(error),
        }
    }
}

/// Request status information for an item in the download queue.
///
/// Provides current status, queue position, progress, and error information.
#[derive(Debug, Clone, Serialize)]
pub struct RequestStatusInfo {
    /// Queue item ID (UUID)
    pub request_id: String,
    /// Current status in the queue
    pub status: QueueStatus,
    /// Position in queue (1-based, only for pending items)
    pub queue_position: Option<usize>,
    /// Download progress (for album downloads with children)
    pub progress: Option<DownloadProgress>,
    /// Error message (for failed items)
    pub error_message: Option<String>,
    /// When the request was created (Unix timestamp)
    pub created_at: i64,
}

impl RequestStatusInfo {
    /// Create a RequestStatusInfo from a QueueItem.
    pub fn from_queue_item(
        item: &QueueItem,
        queue_position: Option<usize>,
        progress: Option<DownloadProgress>,
    ) -> Self {
        Self {
            request_id: item.id.clone(),
            status: item.status,
            queue_position,
            progress,
            error_message: item.error_message.clone(),
            created_at: item.created_at,
        }
    }
}

/// Request to download an album.
#[derive(Debug, Clone, Deserialize)]
pub struct AlbumRequest {
    /// External album ID
    pub album_id: String,
    /// Album name for display
    pub album_name: String,
    /// Artist name for display
    pub artist_name: String,
}

/// Request to download an artist's discography.
#[derive(Debug, Clone, Deserialize)]
pub struct DiscographyRequest {
    /// External artist ID
    pub artist_id: String,
    /// Artist name for display
    pub artist_name: String,
}

/// Result of submitting a download request.
#[derive(Debug, Clone, Serialize)]
pub struct RequestResult {
    /// ID of the created queue item
    pub request_id: String,
    /// Initial status (usually PENDING)
    pub status: QueueStatus,
    /// Position in the queue
    pub queue_position: usize,
}

/// Result of submitting a discography download request.
#[derive(Debug, Clone, Serialize)]
pub struct DiscographyRequestResult {
    /// IDs of the created queue items
    pub request_ids: Vec<String>,
    /// Number of albums queued
    pub albums_queued: usize,
    /// Number of albums skipped (already in catalog)
    pub albums_skipped: usize,
    /// Initial status
    pub status: QueueStatus,
}

/// Execution mode for the missing files watchdog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MissingFilesMode {
    /// Report what would be done without making changes.
    #[default]
    DryRun,
    /// Actually queue download requests.
    Actual,
}

/// Detailed information about a missing track audio file.
#[derive(Debug, Clone, Serialize)]
pub struct MissingTrackInfo {
    /// Track ID (base62)
    pub track_id: String,
    /// Track name
    pub track_name: String,
    /// Album ID the track belongs to
    pub album_id: Option<String>,
    /// Album name
    pub album_name: Option<String>,
    /// Artist names (primary artists)
    pub artist_names: Vec<String>,
}

/// Detailed information about a missing image file.
#[derive(Debug, Clone, Serialize)]
pub struct MissingImageInfo {
    /// Image ID (hex)
    pub image_id: String,
    /// Associated entity ID (album or artist ID)
    pub entity_id: String,
    /// Associated entity name (album or artist name)
    pub entity_name: String,
}

/// Report from the missing files watchdog scan showing missing media files and actions taken.
#[derive(Debug, Default, Clone, Serialize)]
pub struct MissingFilesReport {
    /// The mode that was used for this run.
    pub mode: MissingFilesMode,
    /// Total tracks scanned in the catalog
    pub total_tracks_scanned: usize,
    /// Total album images scanned in the catalog
    pub total_album_images_scanned: usize,
    /// Total artist images scanned in the catalog
    pub total_artist_images_scanned: usize,
    /// Track IDs (base62) for tracks missing audio files
    pub missing_track_audio: Vec<String>,
    /// Detailed info for tracks missing audio files
    pub missing_track_details: Vec<MissingTrackInfo>,
    /// Image IDs (hex) for missing album cover images
    pub missing_album_images: Vec<String>,
    /// Detailed info for missing album images
    pub missing_album_image_details: Vec<MissingImageInfo>,
    /// Image IDs (hex) for missing artist portrait images
    pub missing_artist_images: Vec<String>,
    /// Detailed info for missing artist images
    pub missing_artist_image_details: Vec<MissingImageInfo>,
    /// Number of items queued for download (0 in dry-run mode)
    pub items_queued: usize,
    /// Number of items skipped (already in queue)
    pub items_skipped: usize,
    /// Time taken to complete the scan in milliseconds
    pub scan_duration_ms: i64,
}

impl MissingFilesReport {
    /// Returns the total number of missing items found.
    pub fn total_missing(&self) -> usize {
        self.missing_track_audio.len()
            + self.missing_album_images.len()
            + self.missing_artist_images.len()
    }

    /// Returns true if no missing content was found.
    pub fn is_clean(&self) -> bool {
        self.total_missing() == 0
    }
}

/// Report from the watchdog scan showing missing content and actions taken.
///
/// Deprecated: Use `MissingFilesReport` for missing file scans.
/// This struct is kept for backward compatibility with audit logging.
#[derive(Debug, Default, Clone)]
pub struct WatchdogReport {
    /// Track IDs (base62) for tracks missing audio files
    pub missing_track_audio: Vec<String>,
    /// Image IDs (hex) for missing album cover images
    pub missing_album_images: Vec<String>,
    /// Image IDs (hex) for missing artist portrait images
    pub missing_artist_images: Vec<String>,
    /// Artist IDs that have no related artists populated
    pub artists_without_related: Vec<String>,
    /// Related artist IDs that don't exist in the artists table
    pub orphan_related_artist_ids: Vec<String>,
    /// Number of items queued for download
    pub items_queued: usize,
    /// Number of items skipped (already in queue)
    pub items_skipped: usize,
    /// Time taken to complete the scan in milliseconds
    pub scan_duration_ms: i64,
}

impl WatchdogReport {
    /// Returns the total number of missing items found.
    pub fn total_missing(&self) -> usize {
        self.missing_track_audio.len()
            + self.missing_album_images.len()
            + self.missing_artist_images.len()
    }

    /// Returns the total number of artist enrichment items found.
    pub fn total_artist_enrichment(&self) -> usize {
        self.artists_without_related.len() + self.orphan_related_artist_ids.len()
    }

    /// Returns true if no missing content was found.
    pub fn is_clean(&self) -> bool {
        self.total_missing() == 0 && self.total_artist_enrichment() == 0
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
        assert!(QueuePriority::Urgent < QueuePriority::User);
        assert!(QueuePriority::User < QueuePriority::Expansion);
        assert!(QueuePriority::Expansion < QueuePriority::Background);
    }

    #[test]
    fn test_queue_priority_conversion() {
        assert_eq!(QueuePriority::Urgent.as_i32(), 1);
        assert_eq!(QueuePriority::User.as_i32(), 2);
        assert_eq!(QueuePriority::Expansion.as_i32(), 3);
        assert_eq!(QueuePriority::Background.as_i32(), 4);

        assert_eq!(QueuePriority::from_i32(1), Some(QueuePriority::Urgent));
        assert_eq!(QueuePriority::from_i32(2), Some(QueuePriority::User));
        assert_eq!(QueuePriority::from_i32(3), Some(QueuePriority::Expansion));
        assert_eq!(QueuePriority::from_i32(4), Some(QueuePriority::Background));
        assert_eq!(QueuePriority::from_i32(0), None);
        assert_eq!(QueuePriority::from_i32(5), None);
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
        assert!(DownloadErrorType::Corruption.is_retryable());
        assert!(DownloadErrorType::Unknown.is_retryable());
    }

    #[test]
    fn test_download_error_type_is_corruption() {
        assert!(!DownloadErrorType::Connection.is_corruption());
        assert!(!DownloadErrorType::Parse.is_corruption());
        assert!(DownloadErrorType::Corruption.is_corruption());
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
        .with_names(
            Some("Album Name".to_string()),
            Some("Artist Name".to_string()),
        );

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
        .with_names(
            Some("Test Album".to_string()),
            Some("Test Artist".to_string()),
        );

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

    #[test]
    fn test_audit_event_type_conversion() {
        assert_eq!(AuditEventType::RequestCreated.as_str(), "REQUEST_CREATED");
        assert_eq!(AuditEventType::DownloadStarted.as_str(), "DOWNLOAD_STARTED");
        assert_eq!(AuditEventType::ChildrenCreated.as_str(), "CHILDREN_CREATED");
        assert_eq!(
            AuditEventType::DownloadCompleted.as_str(),
            "DOWNLOAD_COMPLETED"
        );
        assert_eq!(AuditEventType::DownloadFailed.as_str(), "DOWNLOAD_FAILED");

        assert_eq!(
            AuditEventType::from_str("REQUEST_CREATED"),
            Some(AuditEventType::RequestCreated)
        );
        assert_eq!(
            AuditEventType::from_str("WATCHDOG_SCAN_COMPLETED"),
            Some(AuditEventType::WatchdogScanCompleted)
        );
        assert_eq!(AuditEventType::from_str("INVALID"), None);
    }

    #[test]
    fn test_audit_event_type_serialization() {
        let event = AuditEventType::DownloadCompleted;
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(json, "\"DOWNLOAD_COMPLETED\"");

        let deserialized: AuditEventType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, AuditEventType::DownloadCompleted);
    }

    #[test]
    fn test_audit_log_entry_builder() {
        let entry = AuditLogEntry::new(AuditEventType::RequestCreated)
            .with_queue_item("queue-123".to_string())
            .with_content(DownloadContentType::Album, "album-456".to_string())
            .with_user("user-789".to_string())
            .with_source(RequestSource::User)
            .with_details(serde_json::json!({"key": "value"}));

        assert_eq!(entry.event_type, AuditEventType::RequestCreated);
        assert_eq!(entry.queue_item_id, Some("queue-123".to_string()));
        assert_eq!(entry.content_type, Some(DownloadContentType::Album));
        assert_eq!(entry.content_id, Some("album-456".to_string()));
        assert_eq!(entry.user_id, Some("user-789".to_string()));
        assert_eq!(entry.request_source, Some(RequestSource::User));
        assert!(entry.details.is_some());
    }

    #[test]
    fn test_audit_log_filter_builder() {
        let filter = AuditLogFilter::new()
            .for_user("user-123".to_string())
            .with_event_types(vec![
                AuditEventType::RequestCreated,
                AuditEventType::DownloadCompleted,
            ])
            .in_range(Some(1000), Some(2000))
            .paginate(50, 100);

        assert_eq!(filter.user_id, Some("user-123".to_string()));
        assert_eq!(filter.event_types.as_ref().unwrap().len(), 2);
        assert_eq!(filter.since, Some(1000));
        assert_eq!(filter.until, Some(2000));
        assert_eq!(filter.limit, 50);
        assert_eq!(filter.offset, 100);
    }

    #[test]
    fn test_audit_log_filter_default() {
        let filter = AuditLogFilter::new();

        assert_eq!(filter.limit, 100);
        assert_eq!(filter.offset, 0);
        assert!(filter.user_id.is_none());
        assert!(filter.queue_item_id.is_none());
    }

    #[test]
    fn test_user_limit_status_can_request() {
        // Under limits - can request
        let status = UserLimitStatus::available(5, 10, 3, 20);
        assert!(status.can_request);
        assert_eq!(status.requests_today, 5);
        assert_eq!(status.max_per_day, 10);

        // At daily limit - cannot request
        let status = UserLimitStatus::available(10, 10, 3, 20);
        assert!(!status.can_request);

        // At queue limit - cannot request
        let status = UserLimitStatus::available(5, 10, 20, 20);
        assert!(!status.can_request);
    }

    #[test]
    fn test_capacity_status_at_capacity() {
        // Under limits
        let status = CapacityStatus::new(5, 10, 30, 60);
        assert!(!status.at_capacity);

        // At hourly limit
        let status = CapacityStatus::new(10, 10, 30, 60);
        assert!(status.at_capacity);

        // At daily limit
        let status = CapacityStatus::new(5, 10, 60, 60);
        assert!(status.at_capacity);
    }

    #[test]
    fn test_download_error() {
        let error = DownloadError::new(DownloadErrorType::Connection, "Connection refused");
        assert!(error.is_retryable());
        assert_eq!(error.message, "Connection refused");

        let not_found = DownloadError::new(DownloadErrorType::NotFound, "Album not found");
        assert!(!not_found.is_retryable());
    }

    #[test]
    fn test_processing_result_success() {
        let result = ProcessingResult::success(
            "item-123".to_string(),
            DownloadContentType::TrackAudio,
            1024000,
            500,
        );

        assert!(result.success);
        assert_eq!(result.queue_item_id, "item-123");
        assert_eq!(result.content_type, DownloadContentType::TrackAudio);
        assert_eq!(result.bytes_downloaded, Some(1024000));
        assert_eq!(result.duration_ms, 500);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_processing_result_failure() {
        let error = DownloadError::new(DownloadErrorType::Timeout, "Request timed out");
        let result = ProcessingResult::failure(
            "item-123".to_string(),
            DownloadContentType::AlbumImage,
            1000,
            error,
        );

        assert!(!result.success);
        assert!(result.bytes_downloaded.is_none());
        assert!(result.error.is_some());
        assert_eq!(
            result.error.as_ref().unwrap().error_type,
            DownloadErrorType::Timeout
        );
    }

    #[test]
    fn test_queue_stats_default() {
        let stats = QueueStats::default();
        assert_eq!(stats.pending, 0);
        assert_eq!(stats.in_progress, 0);
        assert_eq!(stats.retry_waiting, 0);
        assert_eq!(stats.completed_today, 0);
        assert_eq!(stats.failed_today, 0);
    }

    #[test]
    fn test_album_request_deserialization() {
        let json = r#"{"album_id":"abc123","album_name":"My Album","artist_name":"My Artist"}"#;
        let request: AlbumRequest = serde_json::from_str(json).unwrap();

        assert_eq!(request.album_id, "abc123");
        assert_eq!(request.album_name, "My Album");
        assert_eq!(request.artist_name, "My Artist");
    }

    #[test]
    fn test_discography_request_deserialization() {
        let json = r#"{"artist_id":"xyz789","artist_name":"Some Artist"}"#;
        let request: DiscographyRequest = serde_json::from_str(json).unwrap();

        assert_eq!(request.artist_id, "xyz789");
        assert_eq!(request.artist_name, "Some Artist");
    }

    #[test]
    fn test_request_result_serialization() {
        let result = RequestResult {
            request_id: "req-123".to_string(),
            status: QueueStatus::Pending,
            queue_position: 5,
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"request_id\":\"req-123\""));
        assert!(json.contains("\"status\":\"PENDING\""));
        assert!(json.contains("\"queue_position\":5"));
    }

    #[test]
    fn test_watchdog_report_default() {
        let report = WatchdogReport::default();
        assert!(report.missing_track_audio.is_empty());
        assert!(report.missing_album_images.is_empty());
        assert!(report.missing_artist_images.is_empty());
        assert_eq!(report.items_queued, 0);
        assert_eq!(report.items_skipped, 0);
        assert_eq!(report.scan_duration_ms, 0);
    }

    #[test]
    fn test_watchdog_report_total_missing() {
        let report = WatchdogReport {
            missing_track_audio: vec!["track1".to_string(), "track2".to_string()],
            missing_album_images: vec!["img1".to_string()],
            missing_artist_images: vec!["art1".to_string(), "art2".to_string(), "art3".to_string()],
            artists_without_related: vec![],
            orphan_related_artist_ids: vec![],
            items_queued: 5,
            items_skipped: 1,
            scan_duration_ms: 1500,
        };

        assert_eq!(report.total_missing(), 6); // 2 + 1 + 3
    }

    #[test]
    fn test_watchdog_report_is_clean() {
        let clean_report = WatchdogReport::default();
        assert!(clean_report.is_clean());

        let dirty_report = WatchdogReport {
            missing_track_audio: vec!["track1".to_string()],
            ..Default::default()
        };
        assert!(!dirty_report.is_clean());
    }

    // === Model serialization tests (DM-1.2.8) ===

    #[test]
    fn test_request_source_serialization() {
        // Verify all variants serialize to expected strings
        assert_eq!(
            serde_json::to_string(&RequestSource::User).unwrap(),
            "\"USER\""
        );
        assert_eq!(
            serde_json::to_string(&RequestSource::Watchdog).unwrap(),
            "\"WATCHDOG\""
        );
        assert_eq!(
            serde_json::to_string(&RequestSource::Expansion).unwrap(),
            "\"EXPANSION\""
        );

        // Verify round-trip
        let source = RequestSource::Watchdog;
        let json = serde_json::to_string(&source).unwrap();
        let deserialized: RequestSource = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, source);
    }

    #[test]
    fn test_queue_priority_serialization() {
        // Verify all variants serialize correctly
        let urgent = QueuePriority::Urgent;
        let user = QueuePriority::User;
        let expansion = QueuePriority::Expansion;
        let background = QueuePriority::Background;

        let json_u = serde_json::to_string(&urgent).unwrap();
        let json_user = serde_json::to_string(&user).unwrap();
        let json_e = serde_json::to_string(&expansion).unwrap();
        let json_b = serde_json::to_string(&background).unwrap();

        // Round-trip test
        assert_eq!(
            serde_json::from_str::<QueuePriority>(&json_u).unwrap(),
            urgent
        );
        assert_eq!(
            serde_json::from_str::<QueuePriority>(&json_user).unwrap(),
            user
        );
        assert_eq!(
            serde_json::from_str::<QueuePriority>(&json_e).unwrap(),
            expansion
        );
        assert_eq!(
            serde_json::from_str::<QueuePriority>(&json_b).unwrap(),
            background
        );
    }

    #[test]
    fn test_queue_item_json_round_trip() {
        let item = QueueItem::new(
            "queue-item-123".to_string(),
            DownloadContentType::Album,
            "album-456".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        )
        .with_names(
            Some("Test Album".to_string()),
            Some("Test Artist".to_string()),
        )
        .with_user("user-789".to_string());

        // Serialize to JSON
        let json = serde_json::to_string(&item).unwrap();

        // Verify key fields are present
        assert!(json.contains("\"id\":\"queue-item-123\""));
        assert!(json.contains("\"content_type\":\"ALBUM\""));
        assert!(json.contains("\"content_id\":\"album-456\""));
        assert!(json.contains("\"content_name\":\"Test Album\""));
        assert!(json.contains("\"artist_name\":\"Test Artist\""));
        assert!(json.contains("\"status\":\"PENDING\""));

        // Deserialize back
        let deserialized: QueueItem = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, item.id);
        assert_eq!(deserialized.content_type, item.content_type);
        assert_eq!(deserialized.content_id, item.content_id);
        assert_eq!(deserialized.content_name, item.content_name);
        assert_eq!(deserialized.artist_name, item.artist_name);
        assert_eq!(deserialized.status, item.status);
        assert_eq!(deserialized.priority, item.priority);
        assert_eq!(deserialized.request_source, item.request_source);
        assert_eq!(deserialized.requested_by_user_id, item.requested_by_user_id);
        assert_eq!(deserialized.max_retries, item.max_retries);
    }

    #[test]
    fn test_queue_item_with_optional_fields_json_round_trip() {
        // Create a more complex item with error info
        let mut item = QueueItem::new(
            "failed-item".to_string(),
            DownloadContentType::TrackAudio,
            "track-123".to_string(),
            QueuePriority::Urgent,
            RequestSource::Watchdog,
            3,
        );
        item.status = QueueStatus::Failed;
        item.error_type = Some(DownloadErrorType::NotFound);
        item.error_message = Some("Track not found on provider".to_string());
        item.retry_count = 3;
        item.bytes_downloaded = Some(0);
        item.processing_duration_ms = Some(1500);

        let json = serde_json::to_string(&item).unwrap();
        let deserialized: QueueItem = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.status, QueueStatus::Failed);
        assert_eq!(deserialized.error_type, Some(DownloadErrorType::NotFound));
        assert_eq!(
            deserialized.error_message,
            Some("Track not found on provider".to_string())
        );
        assert_eq!(deserialized.retry_count, 3);
    }

    #[test]
    fn test_audit_log_entry_serialization_basic() {
        let entry = AuditLogEntry::new(AuditEventType::RequestCreated)
            .with_queue_item("queue-123".to_string())
            .with_content(DownloadContentType::Album, "album-456".to_string())
            .with_user("user-789".to_string())
            .with_source(RequestSource::User);

        let json = serde_json::to_string(&entry).unwrap();

        assert!(json.contains("\"event_type\":\"REQUEST_CREATED\""));
        assert!(json.contains("\"queue_item_id\":\"queue-123\""));
        assert!(json.contains("\"content_type\":\"ALBUM\""));
        assert!(json.contains("\"content_id\":\"album-456\""));
        assert!(json.contains("\"user_id\":\"user-789\""));
        assert!(json.contains("\"request_source\":\"USER\""));
    }

    #[test]
    fn test_audit_log_entry_with_object_details() {
        let details = serde_json::json!({
            "child_count": 12,
            "track_ids": ["track-1", "track-2", "track-3"]
        });

        let entry = AuditLogEntry::new(AuditEventType::ChildrenCreated)
            .with_queue_item("parent-123".to_string())
            .with_details(details);

        let json = serde_json::to_string(&entry).unwrap();

        assert!(json.contains("\"event_type\":\"CHILDREN_CREATED\""));
        assert!(json.contains("\"child_count\":12"));
        assert!(json.contains("\"track_ids\":[\"track-1\",\"track-2\",\"track-3\"]"));
    }

    #[test]
    fn test_audit_log_entry_with_error_details() {
        let details = serde_json::json!({
            "error_type": "connection",
            "error_message": "Connection refused",
            "retry_count": 2,
            "next_retry_at": 1700000000
        });

        let entry = AuditLogEntry::new(AuditEventType::RetryScheduled)
            .with_queue_item("retry-item".to_string())
            .with_details(details);

        let json = serde_json::to_string(&entry).unwrap();

        assert!(json.contains("\"event_type\":\"RETRY_SCHEDULED\""));
        assert!(json.contains("\"error_type\":\"connection\""));
        assert!(json.contains("\"retry_count\":2"));
    }

    #[test]
    fn test_audit_log_entry_watchdog_scan_details() {
        let details = serde_json::json!({
            "missing_tracks": 5,
            "missing_images": 3,
            "items_queued": 8,
            "scan_duration_ms": 2500
        });

        let entry = AuditLogEntry::new(AuditEventType::WatchdogScanCompleted)
            .with_source(RequestSource::Watchdog)
            .with_details(details);

        let json = serde_json::to_string(&entry).unwrap();

        assert!(json.contains("\"event_type\":\"WATCHDOG_SCAN_COMPLETED\""));
        assert!(json.contains("\"request_source\":\"WATCHDOG\""));
        assert!(json.contains("\"missing_tracks\":5"));
        assert!(json.contains("\"items_queued\":8"));
    }

    #[test]
    fn test_request_status_info_from_queue_item() {
        let item = QueueItem::new(
            "queue-item-123".to_string(),
            DownloadContentType::Album,
            "album-456".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );

        let progress = DownloadProgress {
            total_children: 10,
            completed: 3,
            failed: 1,
            pending: 4,
            in_progress: 2,
        };

        let status = RequestStatusInfo::from_queue_item(&item, Some(5), Some(progress));

        assert_eq!(status.request_id, "queue-item-123");
        assert_eq!(status.status, QueueStatus::Pending);
        assert_eq!(status.queue_position, Some(5));
        assert!(status.progress.is_some());
        assert_eq!(status.progress.unwrap().total_children, 10);
        assert!(status.error_message.is_none());
    }

    #[test]
    fn test_request_status_info_with_error() {
        let mut item = QueueItem::new(
            "failed-item".to_string(),
            DownloadContentType::Album,
            "album-789".to_string(),
            QueuePriority::User,
            RequestSource::User,
            5,
        );
        item.status = QueueStatus::Failed;
        item.error_message = Some("Download failed: Connection timeout".to_string());

        let status = RequestStatusInfo::from_queue_item(&item, None, None);

        assert_eq!(status.status, QueueStatus::Failed);
        assert!(status.queue_position.is_none());
        assert!(status.progress.is_none());
        assert_eq!(
            status.error_message,
            Some("Download failed: Connection timeout".to_string())
        );
    }

    #[test]
    fn test_request_status_info_serialization() {
        let status = RequestStatusInfo {
            request_id: "req-123".to_string(),
            status: QueueStatus::Pending,
            queue_position: Some(3),
            progress: None,
            error_message: None,
            created_at: 1700000000,
        };

        let json = serde_json::to_string(&status).unwrap();

        assert!(json.contains("\"request_id\":\"req-123\""));
        assert!(json.contains("\"status\":\"PENDING\""));
        assert!(json.contains("\"queue_position\":3"));
        assert!(json.contains("\"created_at\":1700000000"));
    }
}
