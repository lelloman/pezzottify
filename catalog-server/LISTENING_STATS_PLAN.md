# Listening Stats Feature - Server Implementation Plan

## Overview

Implement server-side infrastructure for collecting and querying user listening statistics. Clients will report playback events via API, supporting future offline queue functionality.

## Requirements Summary

- **Data tracked**: play count, duration (seconds), completion status (>90% = complete)
- **Storage**: Individual events (not aggregated)
- **Enhanced fields**: session_id, timestamps, seek_count, pause_count, playback_context
- **Use cases**: User analytics, admin analytics, recommendations
- **Client support**: API designed for offline queue (session_id deduplication)

---

## 1. Database Schema (V6 Migration)

### Table: `listening_events`

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| id | INTEGER | PRIMARY KEY | Auto-increment ID |
| user_id | INTEGER | NOT NULL, FK→user(id) CASCADE | User reference |
| track_id | TEXT | NOT NULL | Track identifier (e.g., "tra_xxxxx") |
| session_id | TEXT | UNIQUE | Client-generated UUID for deduplication |
| started_at | INTEGER | NOT NULL | Unix timestamp when playback started |
| ended_at | INTEGER | | Unix timestamp when playback ended |
| duration_seconds | INTEGER | NOT NULL | Actual listening time (excluding pauses) |
| track_duration_seconds | INTEGER | NOT NULL | Total track duration (for completion calc) |
| completed | INTEGER | NOT NULL, DEFAULT 0 | 1 if >90% played |
| seek_count | INTEGER | DEFAULT 0 | Number of seek operations |
| pause_count | INTEGER | DEFAULT 0 | Number of pause/resume cycles |
| playback_context | TEXT | | "album", "playlist", "track", "search" |
| client_type | TEXT | | "web", "android", "ios" |
| date | INTEGER | NOT NULL | YYYYMMDD format for efficient queries |
| created | INTEGER | DEFAULT NOW | Record creation timestamp |

### Indices

- `idx_listening_events_user_id` on (user_id)
- `idx_listening_events_track_id` on (track_id)
- `idx_listening_events_date` on (date)
- `idx_listening_events_user_date` on (user_id, date)
- `idx_listening_events_session_id` on (session_id) - for deduplication

---

## 2. Data Models

**File:** `src/user/user_models.rs`

```rust
/// Individual listening event
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ListeningEvent {
    pub id: Option<usize>,
    pub user_id: usize,
    pub track_id: String,
    pub session_id: Option<String>,
    pub started_at: u64,
    pub ended_at: Option<u64>,
    pub duration_seconds: u32,
    pub track_duration_seconds: u32,
    pub completed: bool,
    pub seek_count: u32,
    pub pause_count: u32,
    pub playback_context: Option<String>,
    pub client_type: Option<String>,
    pub date: u32,
}

/// Summary for user or platform
#[derive(Serialize, Debug, Clone)]
pub struct ListeningSummary {
    pub user_id: Option<usize>,
    pub total_plays: u64,
    pub total_duration_seconds: u64,
    pub completed_plays: u64,
    pub unique_tracks: u64,
}

/// Per-track statistics
#[derive(Serialize, Debug, Clone)]
pub struct TrackListeningStats {
    pub track_id: String,
    pub play_count: u64,
    pub total_duration_seconds: u64,
    pub completed_count: u64,
    pub unique_listeners: u64,
}

/// User's listening history entry
#[derive(Serialize, Debug, Clone)]
pub struct UserListeningHistoryEntry {
    pub track_id: String,
    pub last_played_at: u64,
    pub play_count: u64,
    pub total_duration_seconds: u64,
}

/// Daily aggregated stats (for admin)
#[derive(Serialize, Debug, Clone)]
pub struct DailyListeningStats {
    pub date: u32,
    pub total_plays: u64,
    pub total_duration_seconds: u64,
    pub completed_plays: u64,
    pub unique_users: u64,
    pub unique_tracks: u64,
}
```

---

## 3. Store Trait

**File:** `src/user/user_store.rs`

```rust
pub trait UserListeningStore: Send + Sync {
    // Recording
    fn record_listening_event(&self, event: ListeningEvent) -> Result<usize>;

    // User queries
    fn get_user_listening_events(
        &self, user_id: usize, start_date: u32, end_date: u32,
        limit: Option<usize>, offset: Option<usize>
    ) -> Result<Vec<ListeningEvent>>;

    fn get_user_listening_summary(
        &self, user_id: usize, start_date: u32, end_date: u32
    ) -> Result<ListeningSummary>;

    fn get_user_listening_history(
        &self, user_id: usize, limit: usize
    ) -> Result<Vec<UserListeningHistoryEntry>>;

    // Admin queries
    fn get_track_listening_stats(
        &self, track_id: &str, start_date: u32, end_date: u32
    ) -> Result<TrackListeningStats>;

    fn get_daily_listening_stats(
        &self, start_date: u32, end_date: u32
    ) -> Result<Vec<DailyListeningStats>>;

    fn get_top_tracks(
        &self, start_date: u32, end_date: u32, limit: usize
    ) -> Result<Vec<TrackListeningStats>>;

    // Maintenance
    fn prune_listening_events(&self, older_than_days: u32) -> Result<usize>;
}
```

Update `FullUserStore` to include `UserListeningStore`:

```rust
pub trait FullUserStore: UserStore + UserBandwidthStore + UserListeningStore {}
impl<T: UserStore + UserBandwidthStore + UserListeningStore> FullUserStore for T {}
```

---

## 4. API Endpoints

### User Endpoints (require AccessCatalog)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/v1/user/listening` | Report listening event(s) |
| GET | `/v1/user/listening/summary` | Get user's listening summary |
| GET | `/v1/user/listening/history` | Get recently played tracks |
| GET | `/v1/user/listening/events` | Get listening events (paginated) |

### Admin Endpoints (require ViewAnalytics)

> **Note:** `ViewAnalytics` is a new permission to be added in `src/user/permissions.rs`. This separates analytics access from user management (`ManagePermissions`), allowing granular control over who can view platform statistics.

| Method | Path | Description |
|--------|------|-------------|
| GET | `/v1/admin/listening/daily` | Daily platform stats |
| GET | `/v1/admin/listening/top-tracks` | Top tracks by play count |
| GET | `/v1/admin/listening/track/{track_id}` | Stats for specific track |
| GET | `/v1/admin/listening/users/{handle}/summary` | Specific user's summary |

### POST `/v1/user/listening` Request Body

```json
{
  "track_id": "tra_xxxxx",
  "session_id": "uuid-v4",
  "started_at": 1732982400,
  "ended_at": 1732982587,
  "duration_seconds": 187,
  "track_duration_seconds": 210,
  "seek_count": 2,
  "pause_count": 1,
  "playback_context": "album",
  "client_type": "android"
}
```

**Deduplication**: If `session_id` already exists, return success without inserting duplicate (idempotent for offline queue retry).

**Response:**

```json
{ "id": 42, "created": true }
```

### Query Parameters for GET Endpoints

- `start_date`: YYYYMMDD format (e.g., 20251101)
- `end_date`: YYYYMMDD format (e.g., 20251130)
- `limit`: Max results (default: 50, max: 500)
- `offset`: Pagination offset (default: 0)

---

## 5. Prometheus Metrics

**File:** `src/server/metrics.rs`

```rust
pub static ref LISTENING_EVENTS_TOTAL: CounterVec = CounterVec::new(
    Opts::new(format!("{PREFIX}_listening_events_total"), "Total listening events recorded"),
    &["client_type", "completed"]
).expect("Failed to create listening_events_total metric");

pub static ref LISTENING_DURATION_SECONDS_TOTAL: CounterVec = CounterVec::new(
    Opts::new(format!("{PREFIX}_listening_duration_seconds_total"), "Total listening duration in seconds"),
    &["client_type"]
).expect("Failed to create listening_duration_seconds_total metric");
```

---

## 6. Implementation Sequence

### Phase 1: Data Layer
1. ~~Add `ViewAnalytics` permission to `permissions.rs`~~ ✅
2. Add data models to `user_models.rs`
3. Add `UserListeningStore` trait to `user_store.rs`
4. Update `FullUserStore` trait
5. Add V6 schema + migration to `sqlite_user_store.rs`
6. Implement `UserListeningStore` for `SqliteUserStore`

### Phase 2: Business Logic
7. Add wrapper methods to `UserManager`
8. Update exports in `user/mod.rs`

### Phase 3: API Layer
9. Add request/response structs to `server.rs`
10. Implement endpoint handlers
11. Register routes with rate limiting

### Phase 4: Metrics & Testing
12. Add Prometheus metrics
13. Write unit tests for store methods
14. Write API integration tests

---

## 7. Critical Files to Modify

| File | Changes |
|------|---------|
| `src/user/permissions.rs` | Add `ViewAnalytics` permission variant |
| `src/user/user_models.rs` | Add ListeningEvent, ListeningSummary, TrackListeningStats, UserListeningHistoryEntry, DailyListeningStats |
| `src/user/user_store.rs` | Add UserListeningStore trait, update FullUserStore |
| `src/user/sqlite_user_store.rs` | Add V6 schema, migration, implement UserListeningStore |
| `src/user/user_manager.rs` | Add wrapper methods |
| `src/user/mod.rs` | Export new types |
| `src/server/server.rs` | Add endpoints, query params, handlers, `require_view_analytics` middleware |
| `src/server/metrics.rs` | Add listening metrics |

---

## 8. Design Decisions

1. **Individual events vs aggregation**: Store individual events to support user history, per-track analytics, and future recommendations. Aggregation can be added later as materialized views if needed.

2. **Session ID for deduplication**: Clients generate UUID per playback session. Server uses UNIQUE constraint + INSERT OR IGNORE for idempotent reporting (supports offline queue retry).

3. **Completion threshold at 90%**: Standard industry practice. Calculated server-side from `duration_seconds / track_duration_seconds >= 0.90`.

4. **Date column (YYYYMMDD)**: Redundant with `started_at` but enables efficient date-range queries without timestamp parsing.

5. **Enhanced fields optional**: seek_count, pause_count, playback_context can be NULL for simple clients or backward compatibility.

---

## 9. Future Client Integration Notes

When implementing web/Android clients, they should:

- Generate session_id (UUID v4) when track starts playing
- Track actual listening time (excluding pauses)
- Report on: track end, track change, app background, stop
- Queue failed reports locally (localStorage/Room) with retry
- Minimum 5-second threshold before creating a report (skip rapid changes)

### Client Payload (Minimal)

```json
{
  "track_id": "tra_xxxxx",
  "duration_seconds": 187,
  "track_duration_seconds": 210
}
```

### Client Payload (Full)

```json
{
  "track_id": "tra_xxxxx",
  "session_id": "550e8400-e29b-41d4-a716-446655440000",
  "started_at": 1732982400,
  "ended_at": 1732982587,
  "duration_seconds": 187,
  "track_duration_seconds": 210,
  "seek_count": 2,
  "pause_count": 1,
  "playback_context": "album",
  "client_type": "web"
}
```
