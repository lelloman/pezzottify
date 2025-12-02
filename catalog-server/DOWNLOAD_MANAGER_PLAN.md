# Download Manager Implementation Plan

## Overview

Replace the current synchronous, inline download approach with an asynchronous queue-based download manager that:
- Collects download requests from multiple sources (user-initiated, admin API, proactive strategies)
- Manages a persistent queue in SQLite
- Processes downloads in the background with retry logic
- Tracks download activity over time
- Supports proactive download strategies when system is idle

**Design Principles:**
- Trait-based for testability (follow `CatalogStore`, `Downloader` patterns)
- Transaction-safe queue operations (follow `ChangeLogStore` batch pattern)
- Modular and extensible (strategy pattern for custom download logic)
- Minimal changes to existing code

## Module Structure

Create new module: `src/download_manager/`

```
download_manager/
├── mod.rs                  # Main API, DownloadManager struct
├── models.rs              # Types, enums, requests/responses
├── queue_store.rs         # DownloadQueueStore trait + SqliteDownloadQueueStore
├── job_processor.rs       # Background processing loop
├── retry_policy.rs        # Exponential backoff logic
├── strategy.rs            # DownloadStrategy trait
└── strategies/
    ├── mod.rs
    ├── complete_content.rs   # Complete incomplete artists/albums
    └── expand_catalog.rs     # Follow related artists/albums
```

## Database Schema

**Separate database:** `download_queue.db` (not in catalog.db)

This keeps the queue operational state separate from catalog persistent data, avoiding lock contention and enabling independent lifecycle management. The database will use the `VersionedSchema` system for migrations, starting at v1.

### Table 1: `download_queue`

Main queue table tracking all download requests.

```sql
CREATE TABLE download_queue (
    id TEXT PRIMARY KEY,              -- UUID
    status TEXT NOT NULL,             -- PENDING, IN_PROGRESS, RETRY_WAITING, COMPLETED, FAILED, CANCELLED
    content_type TEXT NOT NULL,       -- ARTIST, ALBUM, TRACK_METADATA, TRACK_AUDIO, IMAGE
    content_id TEXT NOT NULL,         -- External ID (e.g., "5a2EaR3hamoenG9rDuVn8j")

    -- Source tracking
    request_source TEXT NOT NULL,     -- USER, ADMIN, PROACTIVE_COMPLETE, PROACTIVE_EXPAND
    requested_by_user_id TEXT,        -- User ID if USER/ADMIN source

    -- State management
    created_at INTEGER NOT NULL,      -- Unix timestamp
    started_at INTEGER,               -- When IN_PROGRESS started
    completed_at INTEGER,             -- When reached terminal state
    last_attempt_at INTEGER,          -- Last retry attempt
    next_retry_at INTEGER,            -- When to retry (for RETRY_WAITING)

    -- Retry management
    retry_count INTEGER DEFAULT 0,
    max_retries INTEGER DEFAULT 5,

    -- Error tracking
    error_type TEXT,                  -- connection, timeout, not_found, parse, storage, unknown
    error_message TEXT,

    -- Result tracking
    bytes_downloaded INTEGER,
    processing_duration_ms INTEGER    -- Total time spent downloading
);

CREATE INDEX idx_download_queue_status ON download_queue(status);
CREATE INDEX idx_download_queue_content ON download_queue(content_type, content_id);
CREATE INDEX idx_download_queue_next_retry ON download_queue(next_retry_at) WHERE status = 'RETRY_WAITING';
CREATE INDEX idx_download_queue_requested_by ON download_queue(requested_by_user_id);
```

### Table 2: `download_activity_log`

Hourly activity tracking for proactive strategy decisions.

```sql
CREATE TABLE download_activity_log (
    hour_bucket INTEGER PRIMARY KEY,  -- Unix timestamp truncated to hour (timestamp - timestamp % 3600)
    requests_count INTEGER DEFAULT 0,
    completed_count INTEGER DEFAULT 0,
    failed_count INTEGER DEFAULT 0,
    bytes_downloaded INTEGER DEFAULT 0,
    last_updated_at INTEGER NOT NULL
);
```

### Table 3: `download_strategies`

Configuration and scheduling for proactive strategies.

```sql
CREATE TABLE download_strategies (
    strategy_name TEXT PRIMARY KEY,   -- complete_content, expand_catalog, custom_*
    enabled INTEGER DEFAULT 1,        -- Boolean: 1 = enabled, 0 = disabled
    check_interval_secs INTEGER NOT NULL,  -- How often to check if strategy should run
    inactivity_threshold_secs INTEGER,     -- Run if no activity for X seconds
    last_executed_at INTEGER,         -- Last execution timestamp
    next_execution_at INTEGER,        -- Scheduled next execution

    -- Statistics
    total_executions INTEGER DEFAULT 0,
    total_items_queued INTEGER DEFAULT 0,
    last_items_queued INTEGER DEFAULT 0
);

-- Default strategies
INSERT INTO download_strategies (strategy_name, check_interval_secs, inactivity_threshold_secs)
VALUES
    ('complete_content', 3600, 7200),   -- Check hourly, run if 2hr idle
    ('expand_catalog', 3600, 14400);    -- Check hourly, run if 4hr idle
```

## State Machine

### Queue Item States

```
┌─────────┐
│ PENDING │ ← Initial state (newly queued)
└────┬────┘
     │
     ↓ (background job picks up)
┌──────────────┐
│ IN_PROGRESS  │ ← Currently downloading
└──────┬───────┘
       │
       ├→ Success → [COMPLETED] (terminal)
       │
       └→ Failure (retry_count < max_retries)
           │
           ↓
       ┌───────────────┐
       │ RETRY_WAITING │ ← Exponential backoff delay
       └───────┬───────┘
               │
               ├→ (after backoff) → back to PENDING
               │
               └→ (retry_count >= max_retries) → [FAILED] (terminal)

[CANCELLED] (terminal) ← User/admin cancellation
```

**Terminal States:** COMPLETED, FAILED, CANCELLED

**Error Types:** connection, timeout, not_found, parse, storage, unknown

## Retry Policy

Exponential backoff with maximum retry limit:

```
Attempt 1: immediate
Attempt 2: +60 seconds
Attempt 3: +120 seconds (2 min)
Attempt 4: +240 seconds (4 min)
Attempt 5: +480 seconds (8 min)
...
Max backoff: 3600 seconds (1 hour)
Max retries: 5
```

After max retries → transition to FAILED (terminal state).

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
pub enum ContentType {
    Artist,
    Album,
    TrackMetadata,
    TrackAudio,
    Image,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestSource {
    User,
    Admin,
    ProactiveComplete,
    ProactiveExpand,
}

#[derive(Debug, Clone)]
pub struct QueueItem {
    pub id: String,
    pub status: QueueStatus,
    pub content_type: ContentType,
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
```

## DownloadManager Public API

Main entry point in `mod.rs`:

```rust
pub struct DownloadManager {
    queue_store: Arc<dyn DownloadQueueStore>,
    downloader: Arc<dyn Downloader>,
    catalog_store: Arc<dyn CatalogStore>,
    media_base_path: PathBuf,
    retry_policy: RetryPolicy,
}

impl DownloadManager {
    pub fn new(...) -> Self;

    // Queue operations
    pub async fn submit_download(&self, request: DownloadRequest) -> Result<String>;
    pub async fn get_status(&self, request_id: &str) -> Result<QueueItem>;
    pub async fn cancel_request(&self, request_id: &str) -> Result<()>;
    pub async fn list_user_requests(&self, user_id: &str) -> Result<Vec<QueueItem>>;

    // Admin operations
    pub async fn get_queue_stats(&self) -> Result<QueueStats>;
    pub async fn get_failed_items(&self, limit: usize) -> Result<Vec<QueueItem>>;
    pub async fn get_activity_summary(&self, hours: usize) -> Result<ActivitySummary>;
    pub async fn execute_strategy_now(&self, strategy_name: &str) -> Result<usize>;

    // Background job entry point
    pub async fn process_queue_once(&self) -> Result<ProcessingStats>;
}
```

## DownloadQueueStore Trait (queue_store.rs)

Follow `CatalogStore` pattern - trait + SQLite implementation:

```rust
#[async_trait]
pub trait DownloadQueueStore: Send + Sync {
    // Queue management
    async fn enqueue(&self, item: QueueItem) -> Result<()>;
    async fn get_item(&self, id: &str) -> Result<Option<QueueItem>>;
    async fn list_by_status(&self, status: QueueStatus, limit: usize) -> Result<Vec<QueueItem>>;
    async fn list_by_user(&self, user_id: &str) -> Result<Vec<QueueItem>>;

    // State transitions (atomic)
    async fn transition_to_in_progress(&self, id: &str) -> Result<bool>;
    async fn transition_to_completed(&self, id: &str, bytes: u64, duration_ms: i64) -> Result<()>;
    async fn transition_to_retry_waiting(&self, id: &str, next_retry_at: i64, error: &DownloadError) -> Result<()>;
    async fn transition_to_failed(&self, id: &str, error: &DownloadError) -> Result<()>;
    async fn transition_to_cancelled(&self, id: &str) -> Result<()>;

    // Duplicate detection
    async fn find_duplicate(&self, content_type: ContentType, content_id: &str) -> Result<Option<QueueItem>>;

    // Activity tracking
    async fn record_activity(&self, hour_bucket: i64, completed: bool, bytes: u64) -> Result<()>;
    async fn get_activity_since(&self, since: i64) -> Result<Vec<ActivityLogEntry>>;

    // Statistics
    async fn get_stats(&self) -> Result<QueueStats>;

    // Cleanup
    async fn cleanup_stale_in_progress(&self, stale_threshold_secs: i64) -> Result<usize>;
}

pub struct SqliteDownloadQueueStore {
    conn: Arc<Mutex<rusqlite::Connection>>,
}
```

**Key Implementation Details:**
- Use `BEGIN IMMEDIATE` for all state transitions
- Implement duplicate detection (check for non-terminal items with same content)
- Activity log uses upsert pattern (INSERT OR REPLACE)
- Stale item cleanup: IN_PROGRESS items older than threshold → PENDING

## Background Jobs

Three background tasks spawned in `main.rs`:

### Job 1: Queue Processor (job_processor.rs)

**Interval:** 5 seconds (configurable)

**Logic:**
1. Get next pending item (oldest first) via `transition_to_in_progress` (atomic)
2. If no pending items, check for retry-ready items (`next_retry_at <= now`)
3. Execute download based on content_type:
   - Artist: Call `downloader.get_artist()` + store in catalog
   - Album: Call `downloader.get_album()` + store in catalog
   - TrackMetadata: Call `downloader.get_track()` + store in catalog
   - TrackAudio: Call `downloader.download_track_audio()` + store file
   - Image: Call `downloader.download_image()` + store file
4. On success: `transition_to_completed` with bytes + duration
5. On error:
   - If retry_count < max_retries: `transition_to_retry_waiting` with backoff
   - Else: `transition_to_failed`
6. Record activity in hourly log
7. Update metrics

**Concurrency:** Single-threaded for MVP (one download at a time). Future: configurable parallelism.

### Job 2: Strategy Scheduler

**Interval:** 300 seconds (5 minutes, configurable)

**Logic:**
1. Query `download_strategies` table for enabled strategies
2. For each strategy:
   - Check if `next_execution_at <= now`
   - If yes, execute strategy via trait method
   - Update `last_executed_at`, `next_execution_at`, statistics
3. Strategies return list of items to enqueue

### Job 3: Cleanup Tasks

**Interval:** 600 seconds (10 minutes)

**Logic:**
1. Cleanup stale IN_PROGRESS items (older than 1 hour) → PENDING
2. Archive old COMPLETED items (older than 7 days) - optional
3. Prune activity log (keep last 30 days)

## Download Strategies

### DownloadStrategy Trait (strategy.rs)

```rust
#[async_trait]
pub trait DownloadStrategy: Send + Sync {
    fn name(&self) -> &str;

    async fn should_execute(
        &self,
        queue_store: &dyn DownloadQueueStore,
        catalog_store: &dyn CatalogStore,
    ) -> Result<bool>;

    async fn generate_downloads(
        &self,
        queue_store: &dyn DownloadQueueStore,
        catalog_store: &dyn CatalogStore,
    ) -> Result<Vec<DownloadRequest>>;
}
```

### Built-in Strategy 1: Complete Content (strategies/complete_content.rs)

**Logic:**
1. Query catalog for artists without related_artists
2. Query catalog for albums without tracks
3. Query catalog for tracks without audio files
4. Query catalog for missing images (check filesystem)
5. Generate download requests for missing content

### Built-in Strategy 2: Expand Catalog (strategies/expand_catalog.rs)

**Logic:**
1. Query catalog for related_artist IDs not in catalog
2. Query catalog for related_album IDs not in catalog
3. Generate download requests for missing entities
4. Limit: max N items per execution (prevent queue flooding)

## API Endpoints

Add to `server.rs` router under `/v1/download/`:

### User Endpoints

```rust
// Submit download request
POST /v1/download/submit
Permission: IssueContentDownload
Body: { content_type: "artist", content_id: "abc123" }
Response: { request_id: "uuid", status: "pending", queue_position: 5 }

// Check status
GET /v1/download/status/:request_id
Permission: AccessCatalog (own requests or admin)
Response: QueueItem (full details)

// List user's requests
GET /v1/download/my-requests
Permission: AccessCatalog
Query: ?status=pending (optional filter)
Response: [QueueItem]

// Cancel request
DELETE /v1/download/:request_id
Permission: IssueContentDownload (own requests or admin)
Response: 204 No Content
```

### Admin Endpoints

```rust
// Queue statistics
GET /v1/download/admin/stats
Permission: ViewAnalytics
Response: {
    pending_count: 10,
    in_progress_count: 2,
    retry_waiting_count: 3,
    completed_count: 150,
    failed_count: 5,
    average_processing_time_ms: 2500,
}

// Failed items
GET /v1/download/admin/failed
Permission: ViewAnalytics
Query: ?limit=50
Response: [QueueItem with errors]

// Activity summary
GET /v1/download/admin/activity
Permission: ViewAnalytics
Query: ?hours=24
Response: {
    total_requests: 100,
    completed: 90,
    failed: 10,
    bytes_downloaded: 1073741824,
}

// Execute strategy manually
POST /v1/download/admin/execute-strategy
Permission: EditCatalog
Body: { strategy_name: "complete_content" }
Response: { items_queued: 25 }

// List all strategies
GET /v1/download/admin/strategies
Permission: ViewAnalytics
Response: [{ name, enabled, last_executed_at, ... }]
```

## Integration with Existing Code

### 1. ServerState (server/state.rs)

Add new field:

```rust
pub struct ServerState {
    // ... existing fields ...
    pub download_manager: Option<Arc<DownloadManager>>,
}
```

Initialize in `ServerState::new()` when downloader is available.

### 2. Main Initialization (main.rs)

After creating downloader client:

```rust
// Create download manager
let download_manager = if let (Some(dl), Some(path)) = (&downloader, media_base_path) {
    let queue_db_path = catalog_db_path.parent()
        .unwrap()
        .join("download_queue.db");

    let manager = Arc::new(DownloadManager::new(
        queue_db_path,
        dl.clone(),
        catalog_store.clone(),
        path.clone(),
    )?);

    info!("Download manager initialized");
    Some(manager)
} else {
    None
};

// Spawn background jobs
if let Some(ref dm) = download_manager {
    // Job 1: Queue processor
    let dm_processor = dm.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            interval.tick().await;
            if let Err(e) = dm_processor.process_queue_once().await {
                error!("Queue processor error: {}", e);
            }
        }
    });

    // Job 2: Strategy scheduler
    let dm_strategy = dm.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(300));
        loop {
            interval.tick().await;
            if let Err(e) = dm_strategy.run_strategy_scheduler().await {
                error!("Strategy scheduler error: {}", e);
            }
        }
    });

    // Job 3: Cleanup
    let dm_cleanup = dm.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(600));
        loop {
            interval.tick().await;
            if let Err(e) = dm_cleanup.run_cleanup_tasks().await {
                error!("Cleanup tasks error: {}", e);
            }
        }
    });
}
```

### 3. Router (server/server.rs)

Add download routes to router:

```rust
let app = Router::new()
    // ... existing routes ...
    .nest("/v1/download", download_routes(server_state.clone()));

fn download_routes(state: ServerState) -> Router {
    Router::new()
        .route("/submit", post(submit_download))
        .route("/status/:id", get(get_download_status))
        .route("/my-requests", get(list_my_requests))
        .route("/:id", delete(cancel_download))
        .route("/admin/stats", get(admin_queue_stats))
        .route("/admin/failed", get(admin_failed_items))
        .route("/admin/activity", get(admin_activity_summary))
        .route("/admin/execute-strategy", post(admin_execute_strategy))
        .route("/admin/strategies", get(admin_list_strategies))
        .with_state(state)
}
```

### 4. Metrics (server/metrics.rs)

Add new metrics:

```rust
pub static ref DOWNLOAD_QUEUE_SIZE: GaugeVec = GaugeVec::new(
    Opts::new("pezzottify_download_queue_size", "Current queue size by status"),
    &["status"]
);

pub static ref DOWNLOAD_PROCESSING_DURATION: Histogram = Histogram::with_opts(
    HistogramOpts::new("pezzottify_download_processing_duration_seconds",
                       "Download processing duration")
        .buckets(vec![0.5, 1.0, 2.0, 5.0, 10.0, 30.0, 60.0, 120.0])
);

pub static ref DOWNLOAD_RETRIES_TOTAL: CounterVec = CounterVec::new(
    Opts::new("pezzottify_download_retries_total", "Download retry attempts"),
    &["content_type"]
);

pub static ref DOWNLOAD_QUEUE_PROCESSED_TOTAL: CounterVec = CounterVec::new(
    Opts::new("pezzottify_download_queue_processed_total", "Processed queue items"),
    &["status", "content_type"]
);
```

### 5. Migration from Inline Downloads (Future)

Gradually replace `proxy.ensure_X_complete()` calls with:

```rust
// Old approach (synchronous)
if let Some(ref proxy) = proxy {
    proxy.ensure_artist_complete(&id).await?;
}

// New approach (asynchronous queue)
if let Some(ref dm) = download_manager {
    dm.submit_if_missing(ContentType::Artist, &id, RequestSource::User).await?;
}
```

Keep proxy for now (compatibility), deprecate later.

## Testing Strategy

### Unit Tests

**queue_store.rs:**
- Test each state transition (atomic, transaction-safe)
- Test duplicate detection
- Test activity log upserts
- Test cleanup_stale_in_progress

**retry_policy.rs:**
- Test exponential backoff calculation
- Test max retry limit enforcement
- Test backoff cap (max 1 hour)

**job_processor.rs:**
- Mock `DownloadQueueStore` and `Downloader`
- Test processing success/failure paths
- Test retry logic
- Test error classification

**strategies:**
- Mock `CatalogStore` with incomplete data
- Test should_execute logic (inactivity threshold)
- Test generate_downloads (correct content types)
- Test deduplication (don't queue existing items)

### Integration Tests

**End-to-end flow:**
1. Submit download request via API
2. Background job picks it up
3. Verify state transitions (PENDING → IN_PROGRESS → COMPLETED)
4. Verify catalog/filesystem updated
5. Query status API, verify result

**Retry flow:**
1. Submit request for non-existent content
2. Verify RETRY_WAITING with backoff
3. Verify retry attempts
4. Verify FAILED after max retries

**Strategy execution:**
1. Create incomplete artist in catalog
2. Execute complete_content strategy
3. Verify download request queued
4. Verify completion

### Test Utilities

Create `MockDownloadQueueStore` following `MockDownloader` pattern in proxy.rs.

## Implementation Phases

### Phase 1: Core Infrastructure (MVP)
**Files:** models.rs, queue_store.rs, retry_policy.rs, mod.rs (basic API)
**Tasks:**
- Define types and enums
- Implement SqliteDownloadQueueStore with schema
- Implement RetryPolicy
- Implement DownloadManager basic methods
- Unit tests for queue_store and retry_policy

**Success Criteria:** Can enqueue items, query status, state transitions work.

### Phase 2: Background Job Processor
**Files:** job_processor.rs, main.rs (spawn job)
**Tasks:**
- Implement process_queue_once logic
- Integrate with existing Downloader
- Add metrics
- Test with MockDownloader

**Success Criteria:** Background job processes queue, downloads content, handles retries.

### Phase 3: API Endpoints
**Files:** server.rs (add routes), route handlers
**Tasks:**
- Implement user endpoints (submit, status, cancel)
- Add permission checks
- Response serialization

**Success Criteria:** Users can submit downloads via API, check status.

### Phase 4: Admin Dashboard
**Files:** Admin route handlers
**Tasks:**
- Implement admin stats endpoints
- Implement failed items query
- Implement activity summary
- Add ViewAnalytics permission checks

**Success Criteria:** Admins can monitor queue health.

### Phase 5: Proactive Strategies
**Files:** strategy.rs, strategies/*.rs, main.rs (spawn scheduler)
**Tasks:**
- Implement DownloadStrategy trait
- Implement complete_content strategy
- Implement expand_catalog strategy
- Implement strategy scheduler job
- Add strategy configuration API

**Success Criteria:** Strategies automatically queue downloads during idle periods.

### Phase 6: Polish and Production Readiness
**Tasks:**
- Comprehensive error handling
- Logging improvements
- Performance testing (queue throughput)
- Cleanup job implementation
- Documentation
- Integration tests

**Success Criteria:** Production-ready, handles edge cases, well-tested.

## Edge Cases and Error Handling

**Duplicate Requests:**
- Before enqueuing, check for existing non-terminal item with same content
- If found: return existing request_id (idempotent)

**Concurrent Processing:**
- `transition_to_in_progress` is atomic (SELECT + UPDATE in transaction)
- Only one job can claim an item
- If claim fails, skip to next item

**Crashes/Restarts:**
- Stale IN_PROGRESS items (older than 1 hour) → PENDING
- No data loss (all state in SQLite)
- Metrics reset on restart (acceptable)

**Download Errors:**
- Classify errors: connection, timeout, not_found, parse, storage
- not_found errors → immediate FAILED (no retry)
- connection/timeout → retry with backoff
- Log all errors with context

**Queue Flooding:**
- Strategies limit items per execution (e.g., max 100)
- API rate limiting (future enhancement)
- Priority system (future enhancement)

**File System Issues:**
- Handle missing directories (create parent dirs)
- Handle disk full (mark as storage error, FAILED)
- Validate files after download (size check)

## Critical Files

### New Files (Create)
1. `catalog-server/src/download_manager/mod.rs` - Main API
2. `catalog-server/src/download_manager/models.rs` - Types
3. `catalog-server/src/download_manager/queue_store.rs` - Persistence
4. `catalog-server/src/download_manager/job_processor.rs` - Background job
5. `catalog-server/src/download_manager/retry_policy.rs` - Retry logic
6. `catalog-server/src/download_manager/strategy.rs` - Strategy trait
7. `catalog-server/src/download_manager/strategies/mod.rs`
8. `catalog-server/src/download_manager/strategies/complete_content.rs`
9. `catalog-server/src/download_manager/strategies/expand_catalog.rs`

### Modified Files
1. `catalog-server/src/server/state.rs` - Add download_manager field
2. `catalog-server/src/server/server.rs` - Add routes, handlers
3. `catalog-server/src/server/metrics.rs` - Add metrics
4. `catalog-server/src/main.rs` - Initialize manager, spawn jobs
5. `catalog-server/src/lib.rs` - Export download_manager module

### Reference Files (Read for patterns)
1. `catalog-server/src/catalog_store/changelog.rs` - Queue/batch pattern
2. `catalog-server/src/server/proxy.rs` - Download logic, MockDownloader
3. `catalog-server/src/sqlite_persistence/versioned_schema.rs` - Schema system
4. `catalog-server/src/user/sqlite_user_store.rs` - Complex SQLite store

## Success Metrics

After implementation, the system should achieve:
- **Reliability:** 95%+ download success rate (excluding not_found errors)
- **Performance:** Process queue items within 10 seconds of submission
- **Observability:** Full visibility via metrics and admin APIs
- **Testability:** 80%+ code coverage with unit + integration tests
- **Extensibility:** New strategies can be added in <100 lines of code

## Future Enhancements (Not in MVP)

- Configurable parallelism (N concurrent downloads)
- Priority queue (admin requests first)
- Download progress streaming (WebSocket updates)
- Bulk download API (submit multiple items)
- Retry policy configuration per item
- Download scheduling (start at specific time)
- Bandwidth throttling
- Download history API (paginated completed items)
- Strategy plugin system (load external strategies)
