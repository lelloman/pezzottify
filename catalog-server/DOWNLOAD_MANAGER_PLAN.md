# Download Manager Implementation Plan

## Overview

A queue-based asynchronous download manager that handles content downloads from a generic free music provider. It operates **alongside** (not replacing) the existing live proxy feature, which serves trusted users for real-time catalog browsing.

### Three Main Components

1. **User Content Requests** - Users with `RequestContent` permission can search the music provider and request albums/discographies to be downloaded
2. **Catalog Integrity Watchdog** - Daily scan for missing files in existing catalog, auto-queues repairs
3. **Catalog Expansion Agent** - Smart expansion based on listening stats (Phase 2, deferred)

### Design Principles

- Trait-based for testability (follow `CatalogStore`, `Downloader` patterns)
- Transaction-safe queue operations
- Priority-based processing
- Configurable capacity limits via TOML
- Coexists with existing proxy feature

### Dependencies

- **TOML Configuration System** - for capacity limits, intervals, feature flags
- **Background Jobs System** - for scheduling the integrity watchdog

## Module Structure

```
download_manager/
├── mod.rs                  # Main API, DownloadManager struct
├── models.rs               # Types, enums, requests/responses
├── queue_store.rs          # DownloadQueueStore trait + SqliteDownloadQueueStore
├── audit_logger.rs         # AuditLogger helper for consistent event logging
├── job_processor.rs        # Background queue processing
├── retry_policy.rs         # Exponential backoff logic
├── search_proxy.rs         # Proxy search requests to downloader
└── watchdog.rs             # Integrity watchdog logic
```

## New Permission

### RequestContent

Allows users to:
- Search the music provider via proxy
- Request album downloads
- Request artist discography downloads
- View their request history and status

This is separate from `IssueContentDownload`, which is for the live proxy feature used by trusted users.

```rust
// In permissions.rs
RequestContent = 9,  // Next available ID (after ViewAnalytics=8)
```

**Note:** Permission ID `7` is `ServerAdmin` (renamed from `RebootServer` in the Background Jobs plan).

**Default grants:**
- Admin role: Yes
- Regular role: No (granted individually)

## Database Schema

**Separate database:** `download_queue.db`

Keeps queue operational state separate from catalog data, avoiding lock contention.

### Table 1: `download_queue`

Main queue table tracking all download requests.

```sql
CREATE TABLE download_queue (
    id TEXT PRIMARY KEY,              -- UUID
    status TEXT NOT NULL,             -- PENDING, IN_PROGRESS, RETRY_WAITING, COMPLETED, FAILED, CANCELLED
    priority INTEGER NOT NULL,        -- 1 = highest (watchdog), 2 = user, 3 = expansion
    content_type TEXT NOT NULL,       -- ALBUM, TRACK_AUDIO, ARTIST_IMAGE, ALBUM_IMAGE
    content_id TEXT NOT NULL,         -- External ID from music provider

    -- Source tracking
    request_source TEXT NOT NULL,     -- USER, WATCHDOG, EXPANSION
    requested_by_user_id TEXT,        -- User ID if USER source

    -- State management
    created_at INTEGER NOT NULL,      -- Unix timestamp
    started_at INTEGER,               -- When IN_PROGRESS started
    completed_at INTEGER,             -- When reached terminal state
    last_attempt_at INTEGER,          -- Last attempt timestamp
    next_retry_at INTEGER,            -- When to retry (for RETRY_WAITING)

    -- Retry management
    retry_count INTEGER DEFAULT 0,
    max_retries INTEGER DEFAULT 5,

    -- Error tracking
    error_type TEXT,                  -- connection, timeout, not_found, parse, storage, unknown
    error_message TEXT,

    -- Result tracking
    bytes_downloaded INTEGER,
    processing_duration_ms INTEGER
);

CREATE INDEX idx_queue_status_priority ON download_queue(status, priority, created_at);
CREATE INDEX idx_queue_content ON download_queue(content_type, content_id);
CREATE INDEX idx_queue_user ON download_queue(requested_by_user_id);
CREATE INDEX idx_queue_next_retry ON download_queue(next_retry_at) WHERE status = 'RETRY_WAITING';
```

### Table 2: `download_activity_log`

Tracks download activity for capacity limiting and analytics.

```sql
CREATE TABLE download_activity_log (
    hour_bucket INTEGER PRIMARY KEY,  -- Unix timestamp truncated to hour
    albums_downloaded INTEGER DEFAULT 0,
    tracks_downloaded INTEGER DEFAULT 0,
    images_downloaded INTEGER DEFAULT 0,
    bytes_downloaded INTEGER DEFAULT 0,
    failed_count INTEGER DEFAULT 0,
    last_updated_at INTEGER NOT NULL
);
```

### Table 3: `user_request_stats`

Per-user rate limiting tracking.

```sql
CREATE TABLE user_request_stats (
    user_id TEXT PRIMARY KEY,
    requests_today INTEGER DEFAULT 0,
    requests_in_queue INTEGER DEFAULT 0,
    last_request_date TEXT,           -- YYYY-MM-DD for daily reset
    last_updated_at INTEGER NOT NULL
);
```

### Table 4: `download_audit_log`

Detailed event-by-event audit trail for all download manager activity.

```sql
CREATE TABLE download_audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp INTEGER NOT NULL,           -- Unix timestamp
    event_type TEXT NOT NULL,             -- See AuditEventType enum
    queue_item_id TEXT,                   -- Reference to download_queue.id (nullable for system events)
    content_type TEXT,                    -- ALBUM, TRACK_AUDIO, ARTIST_IMAGE, ALBUM_IMAGE
    content_id TEXT,                      -- External content ID
    user_id TEXT,                         -- Who triggered the event (NULL for system/watchdog)
    request_source TEXT,                  -- USER, WATCHDOG, EXPANSION, ADMIN
    details TEXT                          -- JSON blob for event-specific data
);

CREATE INDEX idx_audit_timestamp ON download_audit_log(timestamp);
CREATE INDEX idx_audit_queue_item ON download_audit_log(queue_item_id);
CREATE INDEX idx_audit_user ON download_audit_log(user_id);
CREATE INDEX idx_audit_event_type ON download_audit_log(event_type);
CREATE INDEX idx_audit_content ON download_audit_log(content_type, content_id);
```

**Event Types:**

| Event Type | Description | Typical Details |
|------------|-------------|-----------------|
| `REQUEST_CREATED` | User or system requested a download | `{ "album_name": "...", "artist_name": "..." }` |
| `DOWNLOAD_STARTED` | Processing began for a queue item | `{ "retry_count": 0 }` |
| `DOWNLOAD_COMPLETED` | Download finished successfully | `{ "bytes_downloaded": 12345, "duration_ms": 5000 }` |
| `DOWNLOAD_FAILED` | Download failed (terminal) | `{ "error_type": "not_found", "error_message": "..." }` |
| `RETRY_SCHEDULED` | Download failed, retry scheduled | `{ "retry_count": 2, "next_retry_at": 1234567890, "error_type": "timeout" }` |
| `REQUEST_CANCELLED` | User cancelled their request | `{ "previous_status": "pending" }` |
| `ADMIN_RETRY` | Admin triggered retry of failed item | `{ "admin_user_id": "..." }` |
| `WATCHDOG_QUEUED` | Integrity watchdog queued repair | `{ "reason": "missing_audio_file" }` |
| `WATCHDOG_SCAN_STARTED` | Watchdog scan began | `{}` |
| `WATCHDOG_SCAN_COMPLETED` | Watchdog scan finished | `{ "items_found": 5, "items_queued": 3, "items_skipped": 2 }` |

**Details JSON Examples:**

```json
// REQUEST_CREATED (user request)
{
    "album_name": "Abbey Road",
    "artist_name": "The Beatles",
    "queue_position": 5
}

// DOWNLOAD_COMPLETED
{
    "bytes_downloaded": 52428800,
    "duration_ms": 12500,
    "tracks_downloaded": 12
}

// DOWNLOAD_FAILED
{
    "error_type": "connection",
    "error_message": "Connection refused",
    "retry_count": 5
}

// RETRY_SCHEDULED
{
    "retry_count": 2,
    "next_retry_at": 1702345678,
    "backoff_secs": 240,
    "error_type": "timeout",
    "error_message": "Request timed out after 300s"
}

// WATCHDOG_QUEUED
{
    "reason": "missing_audio_file",
    "track_id": "abc123",
    "expected_path": "tracks/album_id/track_id.ogg"
}
```

## Priority System

| Priority | Value | Source | Description |
|----------|-------|--------|-------------|
| Highest | 1 | Watchdog | Fix missing files in existing catalog |
| Medium | 2 | User | User-initiated requests |
| Lowest | 3 | Expansion | Smart catalog growth (Phase 2) |

Queue processing order: `ORDER BY priority ASC, created_at ASC`

## Capacity Limits

Configurable via TOML:

```toml
[download_manager]
enabled = true
max_albums_per_hour = 10
max_albums_per_day = 60

[download_manager.user_limits]
max_requests_per_day = 100
max_queue_size = 200
```

**Enforcement:**
- Global limits checked before starting any download
- Per-user limits checked when submitting requests
- Watchdog bypasses user limits but respects global limits

## State Machine

```
┌─────────┐
│ PENDING │ ← Initial state (newly queued)
└────┬────┘
     │
     ↓ (processor picks up by priority)
┌──────────────┐
│ IN_PROGRESS  │ ← Currently downloading
└──────┬───────┘
       │
       ├─→ Success ──────────────────→ [COMPLETED] (terminal)
       │
       └─→ Failure
           │
           ├─→ retry_count < max_retries
           │   │
           │   ↓
           │   ┌───────────────┐
           │   │ RETRY_WAITING │ ← Exponential backoff
           │   └───────┬───────┘
           │           │
           │           └─→ (after backoff) → PENDING
           │
           └─→ retry_count >= max_retries → [FAILED] (terminal)

[CANCELLED] (terminal) ← User cancellation (own requests only)
```

**Terminal States:** COMPLETED, FAILED, CANCELLED

## Retry Policy

Exponential backoff with configurable limits:

```
Attempt 1: immediate
Attempt 2: +60 seconds
Attempt 3: +120 seconds (2 min)
Attempt 4: +240 seconds (4 min)
Attempt 5: +480 seconds (8 min)
...
Max backoff: 3600 seconds (1 hour)
Max retries: 5 (configurable)
```

**Error classification:**
- `not_found` → immediate FAILED (no retry)
- `connection`, `timeout` → retry with backoff
- `parse`, `storage` → retry with backoff
- `unknown` → retry with backoff

## Core Types (models.rs)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueStatus {
    Pending,
    InProgress,
    RetryWaiting,
    Completed,    // terminal
    Failed,       // terminal
    Cancelled,    // terminal
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueuePriority {
    Watchdog = 1,   // highest
    User = 2,
    Expansion = 3,  // lowest
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadContentType {
    Album,          // Full album (metadata + tracks + audio + images)
    TrackAudio,     // Single track audio file
    ArtistImage,    // Artist image
    AlbumImage,     // Album cover art
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestSource {
    User,
    Watchdog,
    Expansion,
}

#[derive(Debug, Clone)]
pub struct QueueItem {
    pub id: String,
    pub status: QueueStatus,
    pub priority: QueuePriority,
    pub content_type: DownloadContentType,
    pub content_id: String,
    pub request_source: RequestSource,
    pub requested_by_user_id: Option<String>,
    pub created_at: i64,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub retry_count: i32,
    pub max_retries: i32,
    pub error_type: Option<String>,
    pub error_message: Option<String>,
    pub bytes_downloaded: Option<u64>,
    pub processing_duration_ms: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct UserRequestView {
    pub id: String,
    pub content_type: DownloadContentType,
    pub content_id: String,
    pub content_name: String,        // Album/artist name for display
    pub status: QueueStatus,
    pub created_at: i64,
    pub completed_at: Option<i64>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditEventType {
    RequestCreated,
    DownloadStarted,
    DownloadCompleted,
    DownloadFailed,
    RetryScheduled,
    RequestCancelled,
    AdminRetry,
    WatchdogQueued,
    WatchdogScanStarted,
    WatchdogScanCompleted,
}

#[derive(Debug, Clone)]
pub struct AuditLogEntry {
    pub id: i64,
    pub timestamp: i64,
    pub event_type: AuditEventType,
    pub queue_item_id: Option<String>,
    pub content_type: Option<DownloadContentType>,
    pub content_id: Option<String>,
    pub user_id: Option<String>,
    pub request_source: Option<RequestSource>,
    pub details: Option<serde_json::Value>,  // Event-specific JSON data
}

#[derive(Debug, Clone)]
pub struct AuditLogFilter {
    pub queue_item_id: Option<String>,
    pub user_id: Option<String>,
    pub event_types: Option<Vec<AuditEventType>>,
    pub content_type: Option<DownloadContentType>,
    pub content_id: Option<String>,
    pub since: Option<i64>,
    pub until: Option<i64>,
    pub limit: usize,
    pub offset: usize,
}
```

## Search Proxy

### Downloader Service Extensions

The downloader service needs new endpoints:

```
GET /search?q=<query>&type=album|artist
GET /artist/<id>/discography
```

### Catalog Server Proxy Endpoints

```rust
// General search - albums and artists
GET /v1/download/search
Query: ?q=<query>&type=album|artist
Permission: RequestContent
Response: {
    results: [
        {
            id: "external_id",
            type: "album",
            name: "Album Name",
            artist_name: "Artist Name",
            image_url: "...",
            in_catalog: true,      // Already downloaded
            in_queue: false,       // Currently queued
        },
        ...
    ]
}

// Artist discography search
GET /v1/download/search/discography/:artist_id
Permission: RequestContent
Response: {
    artist: {
        id: "external_id",
        name: "Artist Name",
        image_url: "...",
        in_catalog: true,
    },
    albums: [
        {
            id: "external_id",
            name: "Album Name",
            year: 2023,
            image_url: "...",
            in_catalog: true,
            in_queue: false,
        },
        ...
    ]
}
```

**Implementation notes:**
- Proxy request to downloader service
- Enrich response with `in_catalog` by checking catalog DB
- Enrich response with `in_queue` by checking queue DB

## API Endpoints

### User Endpoints

```rust
// Search (see Search Proxy section above)
GET /v1/download/search
GET /v1/download/search/discography/:artist_id

// Request album download
POST /v1/download/request/album
Permission: RequestContent
Body: {
    album_id: "external_id",
    album_name: "Album Name",      // For display in request history
    artist_name: "Artist Name"
}
Response: {
    request_id: "uuid",
    status: "pending",
    queue_position: 5
}
Errors:
    - 429: Rate limit exceeded (daily or queue)
    - 409: Already in catalog or queue

// Request full discography download
POST /v1/download/request/discography
Permission: RequestContent
Body: {
    artist_id: "external_id",
    artist_name: "Artist Name"
}
Response: {
    request_ids: ["uuid1", "uuid2", ...],  // One per album
    albums_queued: 5,
    albums_skipped: 3,                      // Already in catalog
    status: "pending"
}
Errors:
    - 429: Rate limit exceeded
    - 409: All albums already in catalog

// List user's requests
GET /v1/download/my-requests
Permission: RequestContent
Query: ?status=pending|completed|failed (optional)
Response: {
    requests: [UserRequestView],
    stats: {
        requests_today: 15,
        max_per_day: 100,
        in_queue: 8,
        max_queue: 200
    }
}

// Get single request status
GET /v1/download/request/:request_id
Permission: RequestContent (own requests only)
Response: UserRequestView

// Cancel request (only pending/retry_waiting)
DELETE /v1/download/request/:request_id
Permission: RequestContent (own requests only)
Response: 204 No Content
Errors:
    - 404: Not found or not owned
    - 409: Cannot cancel (already in progress/completed/failed)
```

### Admin Endpoints

```rust
// Queue statistics
GET /v1/download/admin/stats
Permission: ViewAnalytics
Response: {
    queue: {
        pending: 10,
        in_progress: 2,
        retry_waiting: 3,
        completed_today: 45,
        failed_today: 2
    },
    capacity: {
        albums_this_hour: 7,
        max_per_hour: 10,
        albums_today: 52,
        max_per_day: 60
    },
    processing: {
        average_duration_ms: 2500,
        success_rate_percent: 95.5
    }
}

// Failed items
GET /v1/download/admin/failed
Permission: ViewAnalytics
Query: ?limit=50&offset=0
Response: [QueueItem with error details]

// Retry failed item
POST /v1/download/admin/retry/:request_id
Permission: EditCatalog
Response: { status: "pending" }

// Activity log
GET /v1/download/admin/activity
Permission: ViewAnalytics
Query: ?hours=24
Response: {
    hourly: [
        { hour: "2024-01-15T10:00:00Z", albums: 8, tracks: 45, bytes: 512000000 },
        ...
    ],
    totals: {
        albums: 52,
        tracks: 312,
        bytes: 3200000000
    }
}

// All requests (admin view)
GET /v1/download/admin/requests
Permission: ViewAnalytics
Query: ?status=...&user_id=...&limit=50&offset=0
Response: [QueueItem]

// Audit log - full event history
GET /v1/download/admin/audit
Permission: ViewAnalytics
Query: ?queue_item_id=...&user_id=...&event_type=...&content_type=...&content_id=...&since=...&until=...&limit=100&offset=0
Response: {
    entries: [
        {
            id: 12345,
            timestamp: 1702345678,
            event_type: "DOWNLOAD_COMPLETED",
            queue_item_id: "uuid",
            content_type: "ALBUM",
            content_id: "external_id",
            user_id: "user_uuid",
            request_source: "USER",
            details: { "bytes_downloaded": 52428800, "duration_ms": 12500 }
        },
        ...
    ],
    total_count: 1234,
    has_more: true
}

// Audit log for specific queue item (convenient shorthand)
GET /v1/download/admin/audit/item/:queue_item_id
Permission: ViewAnalytics
Response: {
    queue_item: QueueItem,
    events: [AuditLogEntry]  // All events for this item, chronologically ordered
}

// Audit log for specific user's activity
GET /v1/download/admin/audit/user/:user_id
Permission: ViewAnalytics
Query: ?since=...&until=...&limit=100&offset=0
Response: {
    entries: [AuditLogEntry],
    total_count: 56,
    has_more: false
}
```

## Integrity Watchdog

### Purpose

Daily scan for missing files in existing catalog entries. Queues repairs at highest priority.

### Scans For

1. **Albums with missing track audio files**
   - Query: albums where any track lacks audio file on disk
   - Action: Queue `TrackAudio` downloads for missing tracks

2. **Albums with missing cover art**
   - Query: albums where image file doesn't exist on disk
   - Action: Queue `AlbumImage` downloads

3. **Artists with missing images**
   - Query: artists where image file doesn't exist on disk
   - Action: Queue `ArtistImage` downloads

4. **Tracks without backing audio**
   - Query: tracks where audio file doesn't exist on disk
   - Action: Queue `TrackAudio` downloads

### Implementation

```rust
// In watchdog.rs
pub struct IntegrityWatchdog {
    catalog_store: Arc<dyn CatalogStore>,
    queue_store: Arc<dyn DownloadQueueStore>,
    media_path: PathBuf,
}

impl IntegrityWatchdog {
    /// Run full integrity scan, returns number of items queued
    pub async fn run_scan(&self) -> Result<WatchdogReport> {
        let mut report = WatchdogReport::default();

        // Scan for missing track audio
        report.missing_track_audio = self.scan_missing_track_audio().await?;

        // Scan for missing album images
        report.missing_album_images = self.scan_missing_album_images().await?;

        // Scan for missing artist images
        report.missing_artist_images = self.scan_missing_artist_images().await?;

        // Queue all found issues
        let queued = self.queue_repairs(&report).await?;
        report.items_queued = queued;

        Ok(report)
    }
}

#[derive(Debug, Default)]
pub struct WatchdogReport {
    pub missing_track_audio: Vec<TrackId>,
    pub missing_album_images: Vec<AlbumId>,
    pub missing_artist_images: Vec<ArtistId>,
    pub items_queued: usize,
    pub items_skipped: usize,  // Already in queue
}
```

### Scheduling

Registered as a background job (via Background Jobs System):

```toml
[background_jobs.integrity_watchdog]
enabled = true
interval_hours = 24
start_time = "03:00"  # Run at 3 AM to minimize impact
```

## DownloadManager Public API

```rust
pub struct DownloadManager {
    queue_store: Arc<dyn DownloadQueueStore>,
    downloader: Arc<dyn Downloader>,
    catalog_store: Arc<dyn CatalogStore>,
    media_path: PathBuf,
    config: DownloadManagerConfig,
    retry_policy: RetryPolicy,
}

impl DownloadManager {
    pub fn new(...) -> Result<Self>;

    // Search proxy
    pub async fn search(&self, query: &str, content_type: SearchType) -> Result<SearchResults>;
    pub async fn search_discography(&self, artist_id: &str) -> Result<DiscographyResults>;

    // User requests
    pub async fn request_album(&self, user_id: &str, album: AlbumRequest) -> Result<RequestResult>;
    pub async fn request_discography(&self, user_id: &str, artist_id: &str) -> Result<DiscographyRequestResult>;
    pub async fn get_user_requests(&self, user_id: &str, filter: Option<QueueStatus>) -> Result<Vec<UserRequestView>>;
    pub async fn get_request_status(&self, user_id: &str, request_id: &str) -> Result<UserRequestView>;
    pub async fn cancel_request(&self, user_id: &str, request_id: &str) -> Result<()>;

    // Rate limiting
    pub async fn check_user_limits(&self, user_id: &str) -> Result<UserLimitStatus>;
    pub async fn check_global_capacity(&self) -> Result<CapacityStatus>;

    // Queue processing (called by background job)
    pub async fn process_next(&self) -> Result<Option<ProcessingResult>>;

    // Admin
    pub async fn get_queue_stats(&self) -> Result<QueueStats>;
    pub async fn get_failed_items(&self, limit: usize, offset: usize) -> Result<Vec<QueueItem>>;
    pub async fn retry_failed(&self, request_id: &str) -> Result<()>;
    pub async fn get_activity(&self, hours: usize) -> Result<ActivitySummary>;
}
```

## DownloadQueueStore Trait

```rust
#[async_trait]
pub trait DownloadQueueStore: Send + Sync {
    // Queue management
    async fn enqueue(&self, item: QueueItem) -> Result<()>;
    async fn get_item(&self, id: &str) -> Result<Option<QueueItem>>;
    async fn get_next_pending(&self) -> Result<Option<QueueItem>>;  // By priority, then age
    async fn list_by_user(&self, user_id: &str, status: Option<QueueStatus>) -> Result<Vec<QueueItem>>;

    // State transitions (atomic)
    async fn claim_for_processing(&self, id: &str) -> Result<bool>;  // PENDING → IN_PROGRESS
    async fn mark_completed(&self, id: &str, bytes: u64, duration_ms: i64) -> Result<()>;
    async fn mark_retry_waiting(&self, id: &str, next_retry_at: i64, error: &DownloadError) -> Result<()>;
    async fn mark_failed(&self, id: &str, error: &DownloadError) -> Result<()>;
    async fn mark_cancelled(&self, id: &str) -> Result<()>;

    // Retry handling
    async fn get_retry_ready(&self) -> Result<Vec<QueueItem>>;  // next_retry_at <= now
    async fn promote_retry_to_pending(&self, id: &str) -> Result<()>;

    // Duplicate/existence checks
    async fn find_by_content(&self, content_type: DownloadContentType, content_id: &str) -> Result<Option<QueueItem>>;
    async fn is_in_queue(&self, content_type: DownloadContentType, content_id: &str) -> Result<bool>;

    // User rate limiting
    async fn get_user_stats(&self, user_id: &str) -> Result<UserRequestStats>;
    async fn increment_user_requests(&self, user_id: &str) -> Result<()>;
    async fn decrement_user_queue(&self, user_id: &str) -> Result<()>;

    // Activity tracking
    async fn record_activity(&self, content_type: DownloadContentType, bytes: u64, success: bool) -> Result<()>;
    async fn get_activity_since(&self, since: i64) -> Result<Vec<ActivityLogEntry>>;
    async fn get_hourly_counts(&self) -> Result<HourlyCounts>;
    async fn get_daily_counts(&self) -> Result<DailyCounts>;

    // Statistics
    async fn get_queue_stats(&self) -> Result<QueueStats>;

    // Cleanup
    async fn cleanup_stale_in_progress(&self, stale_threshold_secs: i64) -> Result<usize>;

    // Audit logging
    async fn log_audit_event(&self, event: AuditLogEntry) -> Result<()>;
    async fn get_audit_log(&self, filter: AuditLogFilter) -> Result<(Vec<AuditLogEntry>, usize)>;  // (entries, total_count)
    async fn get_audit_for_item(&self, queue_item_id: &str) -> Result<Vec<AuditLogEntry>>;
    async fn get_audit_for_user(&self, user_id: &str, since: Option<i64>, until: Option<i64>, limit: usize, offset: usize) -> Result<(Vec<AuditLogEntry>, usize)>;
}
```

## Integration Points

### 1. ServerState (server/state.rs)

```rust
pub struct ServerState {
    // ... existing fields ...
    pub download_manager: Option<Arc<DownloadManager>>,
}
```

### 2. Main Initialization (main.rs)

```rust
// After loading TOML config and creating downloader
let download_manager = if config.download_manager.enabled {
    let queue_db_path = data_dir.join("download_queue.db");

    let manager = Arc::new(DownloadManager::new(
        queue_db_path,
        downloader.clone(),
        catalog_store.clone(),
        media_path.clone(),
        config.download_manager.clone(),
    )?);

    info!("Download manager initialized");
    Some(manager)
} else {
    None
};

// Queue processor is a continuous background task
if let Some(ref dm) = download_manager {
    let dm_processor = dm.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            interval.tick().await;
            match dm_processor.process_next().await {
                Ok(Some(result)) => {
                    info!("Processed download: {:?}", result);
                }
                Ok(None) => {
                    // Queue empty, nothing to do
                }
                Err(e) => {
                    error!("Queue processor error: {}", e);
                }
            }
        }
    });
}

// Integrity watchdog registered via Background Jobs System
// (handled separately by that system)
```

### 3. Router (server/server.rs)

```rust
let app = Router::new()
    // ... existing routes ...
    .nest("/v1/download", download_routes(server_state.clone()));

fn download_routes(state: ServerState) -> Router {
    Router::new()
        // Search
        .route("/search", get(search_content))
        .route("/search/discography/:artist_id", get(search_discography))
        // User requests
        .route("/request/album", post(request_album))
        .route("/request/discography", post(request_discography))
        .route("/my-requests", get(list_my_requests))
        .route("/request/:id", get(get_request_status))
        .route("/request/:id", delete(cancel_request))
        // Admin
        .route("/admin/stats", get(admin_stats))
        .route("/admin/failed", get(admin_failed))
        .route("/admin/retry/:id", post(admin_retry))
        .route("/admin/activity", get(admin_activity))
        .route("/admin/requests", get(admin_all_requests))
        // Admin - Audit log
        .route("/admin/audit", get(admin_audit_log))
        .route("/admin/audit/item/:queue_item_id", get(admin_audit_for_item))
        .route("/admin/audit/user/:user_id", get(admin_audit_for_user))
        .with_state(state)
}
```

### 4. Metrics (server/metrics.rs)

```rust
// Queue size by status
pub static ref DOWNLOAD_QUEUE_SIZE: IntGaugeVec = IntGaugeVec::new(
    Opts::new("pezzottify_download_queue_size", "Current queue size"),
    &["status", "priority"]
);

// Processing metrics
pub static ref DOWNLOAD_PROCESSED_TOTAL: IntCounterVec = IntCounterVec::new(
    Opts::new("pezzottify_download_processed_total", "Processed downloads"),
    &["content_type", "result"]  // result: completed, failed, retry
);

pub static ref DOWNLOAD_PROCESSING_DURATION: Histogram = Histogram::with_opts(
    HistogramOpts::new("pezzottify_download_processing_seconds", "Download processing time")
        .buckets(vec![1.0, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0])
);

// Capacity metrics
pub static ref DOWNLOAD_CAPACITY_USED: IntGaugeVec = IntGaugeVec::new(
    Opts::new("pezzottify_download_capacity_used", "Capacity usage"),
    &["period"]  // hourly, daily
);

// User request metrics
pub static ref DOWNLOAD_USER_REQUESTS_TOTAL: IntCounterVec = IntCounterVec::new(
    Opts::new("pezzottify_download_user_requests_total", "User download requests"),
    &["type"]  // album, discography
);

// Audit log metrics
pub static ref DOWNLOAD_AUDIT_EVENTS_TOTAL: IntCounterVec = IntCounterVec::new(
    Opts::new("pezzottify_download_audit_events_total", "Audit log events"),
    &["event_type"]  // request_created, download_started, download_completed, etc.
);
```

## TOML Configuration

```toml
[download_manager]
enabled = true

# Global capacity limits
max_albums_per_hour = 10
max_albums_per_day = 60

# Per-user limits
user_max_requests_per_day = 100
user_max_queue_size = 200

# Processing
process_interval_secs = 5
stale_in_progress_threshold_secs = 3600  # 1 hour

# Retry policy
max_retries = 5
initial_backoff_secs = 60
max_backoff_secs = 3600
backoff_multiplier = 2.0

# Audit log
audit_log_retention_days = 90  # Auto-cleanup entries older than this (0 = keep forever)
```

## Implementation Phases

### Phase 1: Core Infrastructure
**Files:** models.rs, queue_store.rs, audit_logger.rs, retry_policy.rs, mod.rs

**Tasks:**
- Define all types and enums (including audit log types)
- Implement `SqliteDownloadQueueStore` with schema (including audit log table)
- Implement `AuditLogger` helper for consistent event logging
- Implement `RetryPolicy`
- Basic `DownloadManager` struct with queue operations
- Unit tests for queue store, audit logger, and retry policy

**Success Criteria:** Can enqueue items, query by user, state transitions work, audit events are logged.

### Phase 2: Search Proxy
**Files:** search_proxy.rs, downloader client extensions

**Tasks:**
- Add search endpoints to downloader service (separate work)
- Implement search proxy in catalog-server
- Add `in_catalog` / `in_queue` enrichment
- Discography search endpoint

**Success Criteria:** Users can search and see what's already downloaded.

### Phase 3: User Request API
**Files:** Route handlers, mod.rs extensions

**Tasks:**
- Implement request album endpoint
- Implement request discography endpoint
- Implement my-requests listing
- Implement cancel endpoint
- Rate limit enforcement
- Permission checks (`RequestContent`)

**Success Criteria:** Users can request content and track their requests.

### Phase 4: Queue Processor
**Files:** job_processor.rs, main.rs integration

**Tasks:**
- Implement `process_next` logic
- Integrate with existing `Downloader` trait
- Handle all content types (album, track audio, images)
- Retry logic with backoff
- Capacity limit enforcement
- Metrics integration

**Success Criteria:** Queue processes automatically, respects limits, handles failures.

### Phase 5: Integrity Watchdog
**Files:** watchdog.rs, background job registration

**Tasks:**
- Implement catalog scanning logic
- File existence checks
- Queue repair items at high priority
- Register with Background Jobs System
- Watchdog report/logging

**Success Criteria:** Daily scan finds and queues missing content.

### Phase 6: Admin API & Polish
**Files:** Admin route handlers, metrics

**Tasks:**
- Implement all admin endpoints
- Dashboard statistics
- Failed item management
- Activity logs
- Audit log query endpoints (full log, by item, by user)
- Audit log retention cleanup job
- Comprehensive error handling
- Integration tests

**Success Criteria:** Admins can monitor and manage the queue, query full audit history.

### Phase 7 (Future): Expansion Agent
- Smart expansion based on listening stats
- Separate planning required

## Testing Strategy

### Unit Tests

**queue_store.rs:**
- Enqueue and retrieve items
- Priority ordering
- State transitions (atomic)
- User stats tracking
- Activity logging
- Duplicate detection

**audit_logger.rs:**
- Event creation with correct fields
- JSON details serialization
- Timestamp accuracy

**queue_store.rs (audit methods):**
- Log and retrieve audit events
- Filter by queue_item_id, user_id, event_type
- Filter by time range (since/until)
- Pagination (limit/offset)
- Total count accuracy

**retry_policy.rs:**
- Backoff calculation
- Max retry enforcement
- Error type handling

**search_proxy.rs:**
- Result enrichment with catalog status
- Query proxying

**watchdog.rs:**
- Missing file detection
- Queue item generation
- Deduplication

### Integration Tests

**End-to-end request flow:**
1. User submits album request via API
2. Verify rate limits checked
3. Verify item queued with correct priority
4. Process queue
5. Verify album downloaded and stored
6. Query user requests, verify completed status

**Rate limiting:**
1. Submit requests up to daily limit
2. Verify 429 on next request
3. Verify queue limit separately

**Watchdog:**
1. Create album in catalog without audio files
2. Run watchdog scan
3. Verify items queued at priority 1
4. Process and verify files downloaded

**Audit log:**
1. Submit album request
2. Verify REQUEST_CREATED event logged with user_id and details
3. Process queue
4. Verify DOWNLOAD_STARTED and DOWNLOAD_COMPLETED events logged
5. Query audit by queue_item_id, verify full event timeline
6. Query audit by user_id, verify all user's events returned
7. Test pagination and filtering

## Audit Log Integration Points

The following table shows where each audit event type should be logged in the codebase:

| Event Type | Logged In | Trigger |
|------------|-----------|---------|
| `REQUEST_CREATED` | `request_album()`, `request_discography()` route handlers | User submits download request |
| `DOWNLOAD_STARTED` | `job_processor.rs` → `process_next()` | Queue processor picks up item |
| `DOWNLOAD_COMPLETED` | `job_processor.rs` → `process_next()` | Download succeeds |
| `DOWNLOAD_FAILED` | `job_processor.rs` → `process_next()` | Download fails after max retries |
| `RETRY_SCHEDULED` | `job_processor.rs` → `process_next()` | Download fails, retry scheduled |
| `REQUEST_CANCELLED` | `cancel_request()` route handler | User cancels their request |
| `ADMIN_RETRY` | `admin_retry()` route handler | Admin retries failed item |
| `WATCHDOG_QUEUED` | `watchdog.rs` → `queue_repairs()` | Watchdog finds missing content |
| `WATCHDOG_SCAN_STARTED` | `watchdog.rs` → `run_scan()` | Watchdog scan begins |
| `WATCHDOG_SCAN_COMPLETED` | `watchdog.rs` → `run_scan()` | Watchdog scan finishes |

### AuditLogger Helper

To ensure consistent logging, use the `AuditLogger` helper:

```rust
// In audit_logger.rs
pub struct AuditLogger {
    queue_store: Arc<dyn DownloadQueueStore>,
}

impl AuditLogger {
    pub async fn log_request_created(
        &self,
        queue_item: &QueueItem,
        album_name: &str,
        artist_name: &str,
        queue_position: usize,
    ) -> Result<()>;

    pub async fn log_download_started(
        &self,
        queue_item: &QueueItem,
    ) -> Result<()>;

    pub async fn log_download_completed(
        &self,
        queue_item: &QueueItem,
        bytes_downloaded: u64,
        duration_ms: i64,
        tracks_downloaded: Option<usize>,
    ) -> Result<()>;

    pub async fn log_download_failed(
        &self,
        queue_item: &QueueItem,
        error: &DownloadError,
    ) -> Result<()>;

    pub async fn log_retry_scheduled(
        &self,
        queue_item: &QueueItem,
        next_retry_at: i64,
        backoff_secs: u64,
        error: &DownloadError,
    ) -> Result<()>;

    pub async fn log_request_cancelled(
        &self,
        queue_item: &QueueItem,
        cancelled_by_user_id: &str,
    ) -> Result<()>;

    pub async fn log_admin_retry(
        &self,
        queue_item: &QueueItem,
        admin_user_id: &str,
    ) -> Result<()>;

    pub async fn log_watchdog_queued(
        &self,
        queue_item: &QueueItem,
        reason: &str,
        details: serde_json::Value,
    ) -> Result<()>;

    pub async fn log_watchdog_scan_started(&self) -> Result<()>;

    pub async fn log_watchdog_scan_completed(
        &self,
        report: &WatchdogReport,
    ) -> Result<()>;
}
```

## UI Integration (Web/Android)

### Web

**New Components:**
- Download search page (`/download/search`)
- Search results with "Request Download" buttons
- Artist discography modal with bulk request
- My Requests page (`/download/my-requests`)
- Request status cards (queued/downloading/completed/failed)

**Artist Page Enhancement:**
- "Show Full Discography" button (if `RequestContent` permission)
- Opens discography modal with request options

### Android

**New Screens:**
- Download search screen
- My requests screen with status tracking

**Artist Screen Enhancement:**
- "Full Discography" button for users with permission

## Error Handling

**User-facing errors:**
- Clear messages for rate limits (daily/queue)
- Already in catalog/queue feedback
- Download failures with retry status

**System errors:**
- Logged with context
- Metrics for monitoring
- Admin visibility via failed items API

**Recovery:**
- Stale IN_PROGRESS cleanup (1 hour threshold)
- Automatic retry with backoff
- No request expiration (manual cleanup only)

## Success Metrics

- **Reliability:** 95%+ download success rate (excluding not_found)
- **User Experience:** Clear request tracking, timely notifications
- **Capacity:** Respects configured limits without blocking
- **Observability:** Full visibility via metrics and admin API
- **Testability:** 80%+ code coverage
