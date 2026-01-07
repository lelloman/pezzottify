# Pezzottify Download Manager Re-Implementation Plan

## Overview

Re-implement the download manager to fetch missing audio track files from Torrentino (torrent downloader service) and store them on the server. The catalog is read-only Spotify metadata, so this feature only handles audio file acquisition.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    Pezzottify Catalog Server                    │
├─────────────────────────────────────────────────────────────────┤
│  ┌──────────────────┐    ┌──────────────────┐                  │
│  │  Catalog Store   │    │   DownloadMgr    │                  │
│  │  (SQLite)        │◄──►│   (Reimplemented)│                  │
│  │  - Tracks        │    │  - Queue Store   │                  │
│  │  - Artists       │    │  - Processor     │                  │
│  │  - Albums        │    │  - Watchdog      │                  │
│  └──────────────────┘    └──────────────────┘                  │
│           │                       │                             │
│           │                       │                             │
│  ┌────────┴────────┐    ┌────────┴────────┐                    │
│  │  Media Path     │    │   Torrentino    │                    │
│  │  /audio/*       │    │   Client        │                    │
│  └─────────────────┘    └─────────────────┘                    │
└─────────────────────────────────────────────────────────────────┘
```

## Scope

- **Source:** Torrentino service (ticket-based torrent workflow)
- **Destination:** `{media_path}/audio/{sharded_path}/{track_id}.{ext}`
- **Content:** Audio track files only (no catalog expansion)
- **Albums:** Queued as convenience (all tracks in album)

## Implementation Phases

### Phase 1: Database Schema

**File:** `download_manager/schema.rs`

```sql
CREATE TABLE queue_items (
    id TEXT PRIMARY KEY,
    track_id TEXT NOT NULL,
    status TEXT NOT NULL,             -- PENDING, DOWNLOADING, COMPLETED, FAILED
    priority INTEGER NOT NULL,        -- 1=urgent, 2=user, 3=background
    created_at INTEGER NOT NULL,
    started_at INTEGER,
    completed_at INTEGER,
    retry_count INTEGER DEFAULT 0,
    error_type TEXT,
    error_message TEXT,
    torrentino_ticket_id TEXT,
    requested_by_user_id TEXT,
    bytes_downloaded INTEGER,
    processing_duration_ms INTEGER
);

CREATE INDEX idx_queue_items_status ON queue_items(status);
CREATE INDEX idx_queue_items_track ON queue_items(track_id);
```

### Phase 2: Torrentino Client (NEW)

**File:** `download_manager/torrentino_client.rs`

```rust
pub struct TorrentinoClient {
    client: reqwest::Client,
    base_url: String,
}

impl TorrentinoClient {
    /// Create a ticket for downloading a track
    pub async fn create_track_ticket(&self, track: &TrackInfo) -> Result<Ticket>;

    /// Poll ticket status
    pub async fn get_ticket_status(&self, ticket_id: &str) -> Result<TicketStatus>;

    /// Download completed file
    pub async fn download_completed_file(&self, ticket_id: &str) -> Result<Vec<u8>>;

    /// Cancel a ticket
    pub async fn cancel_ticket(&self, ticket_id: &str) -> Result<()>;
}
```

**API Mapping:**

| Torrentino Endpoint | Pezzottify Action |
|--------------------|-------------------|
| POST /tickets | Create download request |
| GET /tickets/{id} | Poll status |
| GET /tickets?state=completed | Find ready downloads |
| DELETE /tickets/{id} | Cancel |
| POST /tickets/{id}/retry | Retry |

### Phase 3: Queue Store (Simplified)

**File:** `download_manager/queue_store.rs`

Keep queue operations, remove catalog ingestion:

```rust
pub trait DownloadQueueStore {
    fn enqueue(&self, item: QueueItem) -> Result<()>;
    fn get_next_pending(&self) -> Result<Option<QueueItem>>;
    fn mark_in_progress(&self, id: &str) -> Result<()>;
    fn mark_completed(&self, id: &str, bytes: u64, duration_ms: i64) -> Result<()>;
    fn mark_failed(&self, id: &str, error: &DownloadError) -> Result<()>;
    fn get_stale_in_progress(&self, threshold_secs: i64) -> Result<Vec<QueueItem>>;
    fn get_user_requests(&self, user_id: &str, limit: usize, offset: usize) -> Result<Vec<QueueItem>>;
    fn get_stats(&self) -> Result<QueueStats>;
}
```

### Phase 4: Download Manager Core (Reimplement)

**File:** `download_manager/manager.rs`

```rust
impl DownloadManager {
    /// Find tracks missing audio files
    pub fn find_missing_audio_tracks(&self) -> Result<Vec<TrackInfo>>;

    /// Queue a single track for download
    pub fn queue_track(&self, track_id: &str, priority: QueuePriority) -> Result<()>;

    /// Queue all tracks in an album
    pub fn queue_album(&self, album_id: &str) -> Result<usize>;

    /// Process next pending download
    pub async fn process_next(&self) -> Result<Option<ProcessingResult>>;

    /// Run the background processor loop
    pub async fn run_processor(&self);
}
```

**Workflow:**
1. Get next pending item from queue
2. Create Torrentino ticket with track metadata
3. Store ticket_id on queue item
4. Poll ticket until completed/failed
5. Download file from Torrentino
6. Write to sharded path: `audio/{id[0:2]}/{id[2:4]}/{id[4:6]}/{id}.{ext}`
7. Validate with ffprobe
8. Mark completed

### Phase 5: Watchdog (Simplified)

**File:** `download_manager/watchdog.rs`

```rust
impl MissingFilesWatchdog {
    /// Scan catalog for tracks missing audio files
    pub fn scan_missing_audio(&self) -> Result<MissingFilesReport>;

    /// Queue all missing tracks for download
    pub fn queue_missing(&self, mode: MissingFilesMode) -> Result<MissingFilesReport>;
}
```

### Phase 6: HTTP API Routes (NEW)

**File:** `server/server.rs`

```rust
fn download_routes(dm: Arc<DownloadManager>) -> Router {
    Router::new()
        // User endpoints
        .route("/v1/download/track/{track_id}", post(queue_track_download))
        .route("/v1/download/album/{album_id}", post(queue_album_download))
        .route("/v1/download/my-requests", get(get_user_requests))
        .route("/v1/download/limits", get(get_user_limits))

        // Admin endpoints
        .route("/v1/download/admin/scan", post(trigger_watchdog_scan))
        .route("/v1/download/admin/queue-all", post(queue_all_missing))
        .route("/v1/download/admin/stats", get(get_download_stats))
        .route("/v1/download/admin/failed", get(get_failed_items))
}
```

### Phase 7: Server Integration

1. **lib.rs:** Uncomment `pub mod download_manager;`
2. **download_manager/mod.rs:** Update exports (remove catalog_ingestion)
3. **main.rs:** Initialize and start DownloadManager with processor task
4. **server/server.rs:** Add download routes to router

## Files to Modify

| File | Action |
|------|--------|
| `lib.rs` | Uncomment `pub mod download_manager;` |
| `download_manager/mod.rs` | Update exports |
| `download_manager/manager.rs` | Reimplement for track-only downloads |
| `download_manager/queue_store.rs` | Simplify for queue-only |
| `download_manager/torrentino_client.rs` | **NEW** |
| `download_manager/watchdog.rs` | Simplify for audio-only |
| `server/server.rs` | Add download routes |
| `main.rs` | Initialize DownloadManager |

## Files to Remove

- `download_manager/catalog_ingestion.rs`
- `download_manager/downloader_types.rs`
- `download_manager/downloader_client.rs`

## Key Design Decisions

1. **Album Queue:** Queue all tracks individually (simpler than parent-child)
2. **Audio Availability:** Filesystem-based check (no schema changes)
3. **Rate Limiting:** Keep per-user limits for fairness
4. **Sharded Path:** `{id[0:2]}/{id[2:4]}/{id[4:6]}/{id}.{ext}`

## Dependencies

None new - all required crates already in use:
- `reqwest` for HTTP client
- `tokio` for async
- `rusqlite` for queue store
- `ffprobe` for validation
