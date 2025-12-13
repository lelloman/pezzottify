# User Notifications System Plan

## Implementation Status

| Feature | Status | Notes |
|---------|--------|-------|
| Overall | ⏳ Not Started | Spec complete, ready for implementation |

---

## Overview

A system for notifying users about events relevant to them. Notifications sync across devices using the existing cursor-based sync mechanism (same pattern as liked content, settings, playlists).

## Scope

**Phase 1**: Album download completed notifications
**Future**: Download failed, new content from followed artists, system announcements

---

## Database Schema

### Notifications Table

```sql
CREATE TABLE user_notifications (
    id TEXT PRIMARY KEY,
    user_id INTEGER NOT NULL,
    notification_type TEXT NOT NULL,
    title TEXT NOT NULL,
    body TEXT,
    data TEXT,                    -- JSON payload for deep linking
    read_at INTEGER,              -- NULL = unread, timestamp = read
    created_at INTEGER NOT NULL,
    FOREIGN KEY (user_id) REFERENCES user(id) ON DELETE CASCADE
);

CREATE INDEX idx_notifications_user ON user_notifications(user_id, created_at DESC);
```

### Constraints

- **Max 100 notifications per user** - when inserting #101, delete the oldest regardless of read state
- **No age-based retention** - only the 100 limit applies

---

## Sync Integration

Follows the existing sync pattern used by liked content, settings, and playlists.

### New Event Types

Added to `UserEvent` enum in `sync_events.rs`:

```rust
#[serde(rename = "notification_created")]
NotificationCreated {
    notification: Notification,
}

#[serde(rename = "notification_read")]
NotificationRead {
    notification_id: String,
    read_at: i64,
}
```

### Full State Response

`GET /v1/sync/state` response extended:

```rust
struct SyncStateResponse {
    seq: i64,
    likes: LikesState,
    settings: Vec<UserSetting>,
    playlists: Vec<PlaylistState>,
    permissions: Vec<Permission>,
    notifications: Vec<Notification>,  // NEW
}
```

### Client Sync Flow

1. **App starts** → `GET /v1/sync/state` → receives all notifications with current `seq`
2. **Notification created server-side** → event logged → WebSocket broadcasts to all user devices
3. **User marks notification read on Device A** → `POST /v1/user/notifications/{id}/read` → event logged → WebSocket broadcasts to Device B
4. **Device B receives WS message** → updates local notification state

---

## Notification Types

### DownloadCompleted (Phase 1)

Triggered when a user's requested album finishes downloading.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadCompletedData {
    pub album_id: String,
    pub album_name: String,      // Denormalized for display without fetch
    pub artist_name: String,     // Denormalized for display without fetch
    pub image_id: Option<String>, // For notification thumbnail
    pub request_id: String,      // Links back to original download request
}
```

**Title**: `"{album_name}" is ready`
**Body**: `"by {artist_name}"`

### Future Types

| Type | Trigger | Data Payload |
|------|---------|--------------|
| DownloadFailed | Album download failed after retries | `{ album_id, album_name, artist_name, request_id, error_reason }` |
| NewRelease | Followed artist released new album | `{ album_id, album_name, artist_id, artist_name, image_id }` |
| SystemAnnouncement | Admin broadcast | `{ announcement_id, action_url? }` |

---

## API Endpoints

### Mark Notification as Read

```
POST /v1/user/notifications/{id}/read
```

- Requires: authenticated session
- Updates `read_at` timestamp
- Logs `NotificationRead` event
- Broadcasts to other devices via WebSocket
- Returns: `200 OK` or `404 Not Found`

### No Separate List Endpoint

Notifications are fetched via the existing sync mechanism:
- Full list: `GET /v1/sync/state`
- Incremental: `GET /v1/sync/events?since={seq}`

---

## Internal Service

### NotificationService

Responsible for creating notifications. Called by other server components (not exposed via HTTP).

```rust
pub trait NotificationService {
    /// Creates a notification, enforces 100 limit, logs event, broadcasts via WS
    fn create_notification(
        &self,
        user_id: usize,
        notification_type: NotificationType,
        title: String,
        body: Option<String>,
        data: serde_json::Value,
    ) -> Result<Notification, NotificationError>;
}
```

### Integration with Download Manager

`QueueProcessor` calls `NotificationService::create_notification()` when:
- Download completes successfully → `DownloadCompleted` notification
- Download fails (future) → `DownloadFailed` notification

---

## Data Models

### Notification

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notification {
    pub id: String,
    pub notification_type: NotificationType,
    pub title: String,
    pub body: Option<String>,
    pub data: serde_json::Value,
    pub read_at: Option<i64>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationType {
    DownloadCompleted,
    // Future: DownloadFailed, NewRelease, SystemAnnouncement
}
```

---

## Implementation Checklist

### Phase 1: Core Infrastructure

- [ ] Add `user_notifications` table schema
- [ ] Create `Notification` and `NotificationType` models
- [ ] Add `NotificationCreated` and `NotificationRead` to `UserEvent` enum
- [ ] Implement `NotificationStore` trait and `SqliteNotificationStore`
- [ ] Implement `NotificationService` with 100-limit enforcement
- [ ] Extend `GET /v1/sync/state` to include notifications
- [ ] Add `POST /v1/user/notifications/{id}/read` endpoint
- [ ] WebSocket broadcast on notification create/read

### Phase 2: Download Integration

- [ ] Add `DownloadCompletedData` payload struct
- [ ] Call `NotificationService` from `QueueProcessor` on download complete
- [ ] E2E tests for download → notification flow

### Phase 3: Android Client

- [ ] Add notification models to `remoteapi` module
- [ ] Update sync state parsing to include notifications
- [ ] Handle `notification_created` and `notification_read` WS events
- [ ] Local storage for notifications
- [ ] UI for notification list/badge
- [ ] WorkManager fallback for background sync (optional)

### Phase 4: Web Client

- [ ] Update sync store to handle notifications
- [ ] Notification dropdown/panel UI
- [ ] Unread badge indicator

---

## Dependencies

- Existing sync infrastructure (`user_events` table, WebSocket)
- Download Manager (for triggering notifications)

## Used By

- Download Manager (completed/failed notifications)
- Future: Expansion Agent, Admin announcements
