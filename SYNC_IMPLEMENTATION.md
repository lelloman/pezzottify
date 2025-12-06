# Multi-Device Sync — Implementation Checklist

This document breaks down the sync plan into small, actionable tasks.

**Legend:**
- `[ ]` Undone
- `[~]` In Progress
- `[x]` Done

---

## Phase 1: Server — Event Log Infrastructure

### 1.1 Database Schema

#### [x] 1.1.1 Add `user_events` table to schema

**Description:** Create the new table for storing user sync events.

**Context:**
- File: `catalog-server/src/user/sqlite_user_store.rs`
- Look for existing table definitions (e.g., `LIKED_CONTENT_TABLE_V_2`)
- Add new versioned table constant

**Sample:**
```rust
const USER_EVENTS_TABLE_V_1: TableDef = TableDef {
    name: "user_events",
    version: 1,
    create_sql: "CREATE TABLE user_events (
        seq INTEGER PRIMARY KEY AUTOINCREMENT,
        user_id INTEGER NOT NULL REFERENCES user(id) ON DELETE CASCADE,
        event_type TEXT NOT NULL,
        payload TEXT NOT NULL,
        server_timestamp INTEGER DEFAULT (cast(strftime('%s','now') as int))
    )",
};
```

#### [x] 1.1.2 Add index on `(user_id, seq)`

**Description:** Create index for efficient event queries by user.

**Context:**
- Add to schema migration or table creation
- Index is critical for `get_events_since` and `get_min_seq` performance

**Sample:**
```sql
CREATE INDEX idx_user_events_user_seq ON user_events(user_id, seq);
```

#### [x] 1.1.3 Register table in schema migrations

**Description:** Ensure table is created on database initialization/upgrade.

**Context:**
- File: `catalog-server/src/user/sqlite_user_store.rs`
- Look for `VersionedSchema` or migration logic
- Add `USER_EVENTS_TABLE_V_1` to the list of managed tables

---

### 1.2 Event Types

#### [x] 1.2.1 Create `sync_events.rs` module file

**Description:** Create new module for sync event type definitions.

**Context:**
- New file: `catalog-server/src/user/sync_events.rs`

**Sample:**
```rust
//! Sync event types for multi-device synchronization.

use serde::{Deserialize, Serialize};
```

#### [x] 1.2.2 Define `UserEvent` enum

**Description:** Define all sync event types with serde tagging.

**Context:**
- File: `catalog-server/src/user/sync_events.rs`
- Use `#[serde(tag = "type", content = "payload")]` for JSON structure

**Sample:**
```rust
use crate::user::permissions::Permission;
use crate::user::settings::UserSetting;
use crate::user::user_models::LikedContentType;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum UserEvent {
    #[serde(rename = "content_liked")]
    ContentLiked {
        content_type: LikedContentType,
        content_id: String,
    },

    #[serde(rename = "content_unliked")]
    ContentUnliked {
        content_type: LikedContentType,
        content_id: String,
    },

    #[serde(rename = "setting_changed")]
    SettingChanged {
        setting: UserSetting,
    },

    #[serde(rename = "playlist_created")]
    PlaylistCreated {
        playlist_id: String,
        name: String,
    },

    #[serde(rename = "playlist_renamed")]
    PlaylistRenamed {
        playlist_id: String,
        name: String,
    },

    #[serde(rename = "playlist_deleted")]
    PlaylistDeleted {
        playlist_id: String,
    },

    #[serde(rename = "playlist_tracks_updated")]
    PlaylistTracksUpdated {
        playlist_id: String,
        track_ids: Vec<String>,
    },

    #[serde(rename = "permission_granted")]
    PermissionGranted {
        permission: Permission,
    },

    #[serde(rename = "permission_revoked")]
    PermissionRevoked {
        permission: Permission,
    },

    #[serde(rename = "permissions_reset")]
    PermissionsReset {
        permissions: Vec<Permission>,
    },
}
```

#### [x] 1.2.3 Define `StoredEvent` struct

**Description:** Wrapper for events with sequence number and timestamp.

**Context:**
- File: `catalog-server/src/user/sync_events.rs`

**Sample:**
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredEvent {
    pub seq: i64,
    #[serde(flatten)]
    pub event: UserEvent,
    pub server_timestamp: i64,
}
```

#### [x] 1.2.4 Add Serialize/Deserialize to `LikedContentType`

**Description:** Ensure `LikedContentType` can be serialized in events.

**Context:**
- File: `catalog-server/src/user/user_models.rs`
- Add `#[derive(Serialize, Deserialize)]` if not present
- Add serde rename attributes for JSON representation

**Sample:**
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LikedContentType {
    Artist,
    Album,
    Track,
    #[serde(other)]
    Unknown,
}
```

#### [x] 1.2.5 Add Serialize/Deserialize to `Permission`

**Description:** Ensure `Permission` can be serialized in events.

**Context:**
- File: `catalog-server/src/user/permissions.rs`
- Check if already has Serialize/Deserialize (it does per the code)
- Verify JSON representation matches expected format

#### [x] 1.2.6 Export `sync_events` module

**Description:** Make sync_events module public.

**Context:**
- File: `catalog-server/src/user/mod.rs`

**Sample:**
```rust
pub mod sync_events;
pub use sync_events::{StoredEvent, UserEvent};
```

#### [x] 1.2.7 Write unit tests for event serialization

**Description:** Verify events serialize/deserialize correctly.

**Context:**
- File: `catalog-server/src/user/sync_events.rs` (add `#[cfg(test)]` module)

**Sample:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_liked_serialization() {
        let event = UserEvent::ContentLiked {
            content_type: LikedContentType::Album,
            content_id: "album_123".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("content_liked"));
        assert!(json.contains("album"));

        let parsed: UserEvent = serde_json::from_str(&json).unwrap();
        // verify roundtrip
    }
}
```

---

### 1.3 Event Log Storage

#### [x] 1.3.1 Implement `append_event` method

**Description:** Insert a new event into the log and return its sequence number.

**Context:**
- File: `catalog-server/src/user/sqlite_user_store.rs`
- Add method to `SqliteUserStore` impl

**Sample:**
```rust
pub fn append_event(&self, user_id: usize, event: &UserEvent) -> Result<i64> {
    let conn = self.conn.lock().unwrap();
    let payload = serde_json::to_string(event)?;
    let event_type = match event {
        UserEvent::ContentLiked { .. } => "content_liked",
        UserEvent::ContentUnliked { .. } => "content_unliked",
        // ... etc
    };

    conn.execute(
        "INSERT INTO user_events (user_id, event_type, payload) VALUES (?1, ?2, ?3)",
        params![user_id, event_type, payload],
    )?;

    Ok(conn.last_insert_rowid())
}
```

#### [x] 1.3.2 Implement `get_events_since` method

**Description:** Retrieve events after a given sequence number.

**Context:**
- File: `catalog-server/src/user/sqlite_user_store.rs`

**Sample:**
```rust
pub fn get_events_since(&self, user_id: usize, since_seq: i64) -> Result<Vec<StoredEvent>> {
    let conn = self.conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT seq, event_type, payload, server_timestamp
         FROM user_events
         WHERE user_id = ?1 AND seq > ?2
         ORDER BY seq ASC"
    )?;

    let events = stmt.query_map(params![user_id, since_seq], |row| {
        let seq: i64 = row.get(0)?;
        let payload: String = row.get(2)?;
        let server_timestamp: i64 = row.get(3)?;
        let event: UserEvent = serde_json::from_str(&payload)
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(...))?;
        Ok(StoredEvent { seq, event, server_timestamp })
    })?.collect::<Result<Vec<_>, _>>()?;

    Ok(events)
}
```

#### [x] 1.3.3 Implement `get_current_seq` method

**Description:** Get the latest sequence number for a user (0 if none).

**Context:**
- File: `catalog-server/src/user/sqlite_user_store.rs`

**Sample:**
```rust
pub fn get_current_seq(&self, user_id: usize) -> Result<i64> {
    let conn = self.conn.lock().unwrap();
    let seq: Option<i64> = conn.query_row(
        "SELECT MAX(seq) FROM user_events WHERE user_id = ?1",
        params![user_id],
        |row| row.get(0),
    ).optional()?;

    Ok(seq.unwrap_or(0))
}
```

#### [x] 1.3.4 Implement `get_min_seq` method

**Description:** Get the minimum available sequence number for a user.

**Context:**
- File: `catalog-server/src/user/sqlite_user_store.rs`
- Used to detect if requested sequence has been pruned

**Sample:**
```rust
pub fn get_min_seq(&self, user_id: usize) -> Result<Option<i64>> {
    let conn = self.conn.lock().unwrap();
    conn.query_row(
        "SELECT MIN(seq) FROM user_events WHERE user_id = ?1",
        params![user_id],
        |row| row.get(0),
    ).optional()
}
```

#### [x] 1.3.5 Implement `prune_events_older_than` method

**Description:** Delete events older than a given timestamp.

**Context:**
- File: `catalog-server/src/user/sqlite_user_store.rs`

**Sample:**
```rust
pub fn prune_events_older_than(&self, before_timestamp: i64) -> Result<u64> {
    let conn = self.conn.lock().unwrap();
    let deleted = conn.execute(
        "DELETE FROM user_events WHERE server_timestamp < ?1",
        params![before_timestamp],
    )?;
    Ok(deleted as u64)
}
```

#### [x] 1.3.6 Write unit tests for event log storage

**Description:** Test append, retrieve, and prune operations.

**Context:**
- File: `catalog-server/src/user/sqlite_user_store.rs` (in test module)

---

### 1.4 Event Logging in Handlers

#### [x] 1.4.1 Add event logging to like content handler

**Description:** Append `ContentLiked` event after successful like.

**Context:**
- File: `catalog-server/src/server/server.rs`
- Find handler for `POST /v1/user/liked/{type}/{id}`

#### [x] 1.4.2 Add event logging to unlike content handler

**Description:** Append `ContentUnliked` event after successful unlike.

**Context:**
- File: `catalog-server/src/server/server.rs`
- Find handler for `DELETE /v1/user/liked/{type}/{id}`

#### [x] 1.4.3 Add event logging to settings handler

**Description:** Append `SettingChanged` event after successful setting update.

**Context:**
- File: `catalog-server/src/server/server.rs`
- Find handler for `PUT /v1/user/settings`

#### [x] 1.4.4 Add event logging to create playlist handler

**Description:** Append `PlaylistCreated` event after successful creation.

**Context:**
- File: `catalog-server/src/server/server.rs`
- Find `post_playlist` handler

#### [x] 1.4.5 Add event logging to rename playlist handler

**Description:** Append `PlaylistRenamed` event after successful rename.

**Context:**
- File: `catalog-server/src/server/server.rs`
- Find `put_playlist` handler

#### [x] 1.4.6 Add event logging to delete playlist handler

**Description:** Append `PlaylistDeleted` event after successful deletion.

**Context:**
- File: `catalog-server/src/server/server.rs`
- Find `delete_playlist` handler

#### [x] 1.4.7 Add event logging to add tracks handler

**Description:** Append `PlaylistTracksUpdated` event after adding tracks.

**Context:**
- File: `catalog-server/src/server/server.rs`
- Find `add_playlist_tracks` handler
- Need to fetch updated track list for the event

#### [x] 1.4.8 Add event logging to remove tracks handler

**Description:** Append `PlaylistTracksUpdated` event after removing tracks.

**Context:**
- File: `catalog-server/src/server/server.rs`
- Find `remove_tracks_from_playlist` handler
- Need to fetch updated track list for the event

#### [x] 1.4.9 Add event logging to CLI for permission changes

**Description:** Log permission events when admin changes user permissions via CLI.

**Context:**
- File: `catalog-server/src/bin/cli-auth.rs`
- Find permission grant/revoke commands
- Log `PermissionGranted`, `PermissionRevoked`, or `PermissionsReset` events

---

## Phase 2: Server — Sync API Endpoints

### 2.1 Full State Endpoint

#### [x] 2.1.1 Create `SyncStateResponse` struct

**Description:** Define response structure for full state endpoint.

**Context:**
- File: `catalog-server/src/server/server.rs` (or new sync module)

**Sample:**
```rust
#[derive(Serialize)]
struct SyncStateResponse {
    seq: i64,
    likes: LikesState,
    settings: Vec<UserSetting>,
    playlists: Vec<PlaylistState>,
    permissions: Vec<Permission>,
}

#[derive(Serialize)]
struct LikesState {
    albums: Vec<String>,
    artists: Vec<String>,
    tracks: Vec<String>,
}

#[derive(Serialize)]
struct PlaylistState {
    id: String,
    name: String,
    tracks: Vec<String>,
}
```

#### [x] 2.1.2 Implement `get_sync_state` handler

**Description:** Handler that returns full user state with current sequence.

**Context:**
- File: `catalog-server/src/server/server.rs`
- Route: `GET /v1/sync/state`
- Combines: liked content, settings, playlists, permissions

#### [x] 2.1.3 Add route for `GET /v1/sync/state`

**Description:** Wire up the sync state endpoint.

**Context:**
- File: `catalog-server/src/server/server.rs`
- Add to router with `require_access_catalog` middleware

---

### 2.2 Events Since Endpoint

#### [x] 2.2.1 Create `SyncEventsResponse` struct

**Description:** Define response structure for events endpoint.

**Context:**
- File: `catalog-server/src/server/server.rs`

**Sample:**
```rust
#[derive(Serialize)]
struct SyncEventsResponse {
    events: Vec<StoredEvent>,
    current_seq: i64,
}
```

#### [x] 2.2.2 Create `SyncEventsQuery` struct

**Description:** Define query parameters for events endpoint.

**Context:**
- File: `catalog-server/src/server/server.rs`

**Sample:**
```rust
#[derive(Deserialize)]
struct SyncEventsQuery {
    since: i64,
}
```

#### [x] 2.2.3 Implement `get_sync_events` handler

**Description:** Handler that returns events since a sequence number.

**Context:**
- File: `catalog-server/src/server/server.rs`
- Route: `GET /v1/sync/events?since={seq}`
- Return 410 Gone if sequence is pruned

**Sample:**
```rust
async fn get_sync_events(
    session: Session,
    State(user_manager): State<...>,
    Query(query): Query<SyncEventsQuery>,
) -> Response {
    let min_seq = user_manager.get_min_seq(session.user_id)?;

    if let Some(min) = min_seq {
        if query.since < min {
            return (StatusCode::GONE, Json(json!({
                "error": "events_pruned",
                "message": "Requested sequence is no longer available."
            }))).into_response();
        }
    }

    let events = user_manager.get_events_since(session.user_id, query.since)?;
    let current_seq = user_manager.get_current_seq(session.user_id)?;

    Json(SyncEventsResponse { events, current_seq }).into_response()
}
```

#### [x] 2.2.4 Add route for `GET /v1/sync/events`

**Description:** Wire up the sync events endpoint.

**Context:**
- File: `catalog-server/src/server/server.rs`
- Add to router with `require_access_catalog` middleware

#### [x] 2.2.5 Write integration tests for sync endpoints

**Description:** Test both sync endpoints with various scenarios.

**Context:**
- Test full state returns correct data
- Test events returns events in order
- Test 410 for pruned sequences

---

## Phase 3: Server — Real-Time Push via WebSocket

### 3.1 WebSocket Message Types

#### [x] 3.1.1 Add `SYNC` message type constant

**Description:** Add constant for sync message type.

**Context:**
- File: `catalog-server/src/server/websocket/messages.rs`

**Sample:**
```rust
pub mod msg_types {
    // ... existing
    pub const SYNC: &str = "sync";
}
```

#### [x] 3.1.2 Create `SyncMessage` payload struct

**Description:** Define the sync message payload structure.

**Context:**
- File: `catalog-server/src/server/websocket/messages.rs`

**Sample:**
```rust
pub mod sync {
    use serde::{Deserialize, Serialize};
    use crate::user::sync_events::StoredEvent;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct SyncEventMessage {
        pub event: StoredEvent,
    }
}
```

---

### 3.2 Broadcast to Other Devices

#### [x] 3.2.1 Implement `broadcast_to_others` method

**Description:** Broadcast message to all user's devices except source.

**Context:**
- File: `catalog-server/src/server/websocket/connection.rs`
- Add method to `ConnectionManager`

**Sample:**
```rust
pub async fn broadcast_to_others(
    &self,
    user_id: usize,
    source_device_id: usize,
    message: ServerMessage,
) {
    let connections = self.connections.read().await;
    if let Some(user_connections) = connections.get(&user_id) {
        for (device_id, sender) in user_connections {
            if *device_id != source_device_id {
                let _ = sender.send(message.clone());
            }
        }
    }
}
```

---

### 3.3 Broadcast on State Change

#### [x] 3.3.1 Add ConnectionManager to handler state

**Description:** Make ConnectionManager available to HTTP handlers.

**Context:**
- File: `catalog-server/src/server/server.rs`
- Add to app state / extension

#### [x] 3.3.2 Update like handler to broadcast

**Description:** Broadcast sync event after logging like.

**Context:**
- File: `catalog-server/src/server/server.rs`
- After `append_event`, call `broadcast_to_others`

**Sample:**
```rust
// After successful like and event logging:
if let Some(device_id) = session.device_id {
    let ws_msg = ServerMessage::new(
        msg_types::SYNC,
        sync::SyncEventMessage { event: stored_event },
    );
    connection_manager.broadcast_to_others(session.user_id, device_id, ws_msg).await;
}
```

#### [x] 3.3.3 Update unlike handler to broadcast

**Description:** Broadcast sync event after logging unlike.

#### [x] 3.3.4 Update settings handler to broadcast

**Description:** Broadcast sync event after logging setting change.

#### [x] 3.3.5 Update playlist create handler to broadcast

**Description:** Broadcast sync event after logging playlist creation.

#### [x] 3.3.6 Update playlist rename handler to broadcast

**Description:** Broadcast sync event after logging playlist rename.

#### [x] 3.3.7 Update playlist delete handler to broadcast

**Description:** Broadcast sync event after logging playlist deletion.

#### [x] 3.3.8 Update add tracks handler to broadcast

**Description:** Broadcast sync event after logging track addition.

#### [x] 3.3.9 Update remove tracks handler to broadcast

**Description:** Broadcast sync event after logging track removal.

---

## Phase 4: Web Frontend — Sync Client

### 4.1 User Store Updates

#### [x] 4.1.1 Add `likedTrackIds` state

**Description:** Add state for liked track IDs.

**Context:**
- File: `web/src/store/user.js`
- Add `const likedTrackIds = ref(null);`

#### [x] 4.1.2 Add `permissions` state

**Description:** Add state for user permissions.

**Context:**
- File: `web/src/store/user.js`
- Add `const permissions = ref([]);`

#### [x] 4.1.3 Add `applyContentLiked` method

**Description:** Apply a content liked event to local state.

**Context:**
- File: `web/src/store/user.js`

#### [x] 4.1.4 Add `applyContentUnliked` method

**Description:** Apply a content unliked event to local state.

**Context:**
- File: `web/src/store/user.js`

#### [x] 4.1.5 Add `applySettingChanged` method

**Description:** Apply a setting changed event to local state.

**Context:**
- File: `web/src/store/user.js`

#### [x] 4.1.6 Add `applyPlaylistCreated` method

**Description:** Apply a playlist created event to local state.

**Context:**
- File: `web/src/store/user.js`

#### [x] 4.1.7 Add `applyPlaylistRenamed` method

**Description:** Apply a playlist renamed event to local state.

**Context:**
- File: `web/src/store/user.js`

#### [x] 4.1.8 Add `applyPlaylistDeleted` method

**Description:** Apply a playlist deleted event to local state.

**Context:**
- File: `web/src/store/user.js`

#### [x] 4.1.9 Add `applyPlaylistTracksUpdated` method

**Description:** Apply a playlist tracks updated event to local state.

**Context:**
- File: `web/src/store/user.js`

#### [x] 4.1.10 Add `applyPermissionGranted` method

**Description:** Apply a permission granted event to local state.

**Context:**
- File: `web/src/store/user.js`

#### [x] 4.1.11 Add `applyPermissionRevoked` method

**Description:** Apply a permission revoked event to local state.

**Context:**
- File: `web/src/store/user.js`

#### [x] 4.1.12 Add `applyPermissionsReset` method

**Description:** Apply a permissions reset event to local state.

**Context:**
- File: `web/src/store/user.js`

#### [x] 4.1.13 Add setter methods for full sync

**Description:** Add `setLikedAlbums`, `setLikedArtists`, `setLikedTracks`, `setAllSettings`, `setPlaylists`, `setPermissions`.

**Context:**
- File: `web/src/store/user.js`

#### [x] 4.1.14 Export new state and methods

**Description:** Add new state and methods to the store's return object.

**Context:**
- File: `web/src/store/user.js`

---

### 4.2 Remote Store Updates

#### [x] 4.2.1 Add `fetchSyncState` method

**Description:** Fetch full sync state from server.

**Context:**
- File: `web/src/store/remote.js`

**Sample:**
```javascript
const fetchSyncState = async () => {
  const response = await axios.get(`${baseUrl}/v1/sync/state`);
  return response.data;
};
```

#### [x] 4.2.2 Add `fetchSyncEvents` method

**Description:** Fetch sync events since a sequence number.

**Context:**
- File: `web/src/store/remote.js`

**Sample:**
```javascript
const fetchSyncEvents = async (since) => {
  const response = await axios.get(`${baseUrl}/v1/sync/events`, {
    params: { since }
  });
  return response.data;
};
```

#### [x] 4.2.3 Export new methods

**Description:** Add new methods to the store's return object.

**Context:**
- File: `web/src/store/remote.js`

---

### 4.3 Sync Store

#### [x] 4.3.1 Create `sync.js` store file

**Description:** Create the new sync store.

**Context:**
- New file: `web/src/store/sync.js`

#### [x] 4.3.2 Implement cursor persistence

**Description:** Load/save/clear cursor from localStorage.

**Context:**
- File: `web/src/store/sync.js`
- Key format: `sync_cursor_{userId}`

#### [x] 4.3.3 Implement `fullSync` function

**Description:** Fetch full state and update user store.

**Context:**
- File: `web/src/store/sync.js`

#### [x] 4.3.4 Implement `catchUp` function

**Description:** Fetch and apply events since cursor.

**Context:**
- File: `web/src/store/sync.js`
- Handle 410 response by calling fullSync
- Detect sequence gaps and trigger fullSync

#### [x] 4.3.5 Implement `applyEvent` function

**Description:** Dispatch event to appropriate user store method.

**Context:**
- File: `web/src/store/sync.js`
- Switch on event type, call corresponding apply method

#### [x] 4.3.6 Implement WebSocket connection

**Description:** Connect to WebSocket and handle sync messages.

**Context:**
- File: `web/src/store/sync.js`
- Handle reconnection with catch-up

#### [x] 4.3.7 Implement `initialize` function

**Description:** Load cursor, catch up, and connect.

**Context:**
- File: `web/src/store/sync.js`

#### [x] 4.3.8 Implement `cleanup` function

**Description:** Disconnect and clear cursor (for logout).

**Context:**
- File: `web/src/store/sync.js`

#### [x] 4.3.9 Export store

**Description:** Export the sync store with all necessary state and methods.

**Context:**
- File: `web/src/store/sync.js`

---

### 4.4 Integration

#### [x] 4.4.1 Update user store initialize to use sync

**Description:** Replace direct fetch calls with sync initialization.

**Context:**
- File: `web/src/store/user.js`
- Modify `initialize()` to call `syncStore.initialize()`

#### [x] 4.4.2 Update logout flow to cleanup sync

**Description:** Call sync cleanup on logout.

**Context:**
- File: `web/src/store/auth.js` or logout handler
- Call `syncStore.cleanup()` before clearing auth

---

## Phase 5: Android — Sync Client

### 5.1 Sync Event Models

#### [x] 5.1.1 Create `SyncEvent.kt` with event payload classes

**Description:** Define Kotlin data classes for sync events.

**Context:**
- New file: `android/domain/src/main/java/com/lelloman/pezzottify/android/domain/sync/SyncEvent.kt`
- Use kotlinx.serialization annotations

#### [x] 5.1.2 Create `StoredEvent` data class

**Description:** Wrapper with seq, event, and timestamp.

**Context:**
- File: `android/domain/.../sync/SyncEvent.kt`

---

### 5.2 Sync State Store

#### [x] 5.2.1 Create `SyncStateStore` interface

**Description:** Interface for cursor persistence.

**Context:**
- New file: `android/domain/src/main/java/com/lelloman/pezzottify/android/domain/sync/SyncStateStore.kt`

#### [x] 5.2.2 Implement `SyncStateStoreImpl` in localdata

**Description:** Implementation using EncryptedSharedPreferences.

**Context:**
- New file: `android/localdata/src/main/java/.../sync/SyncStateStoreImpl.kt`

#### [x] 5.2.3 Add DI bindings for SyncStateStore

**Description:** Provide SyncStateStore via Hilt.

**Context:**
- File: `android/localdata/.../LocalDataModule.kt` or similar

---

### 5.3 Sync API

#### [x] 5.3.1 Add `getSyncState` to RemoteApiClient

**Description:** Add method to fetch full sync state.

**Context:**
- File: `android/remoteapi/.../RemoteApiClient.kt`

#### [x] 5.3.2 Add `getSyncEvents` to RemoteApiClient

**Description:** Add method to fetch events since sequence.

**Context:**
- File: `android/remoteapi/.../RemoteApiClient.kt`

#### [x] 5.3.3 Create response data classes

**Description:** Define `SyncStateResponse` and `SyncEventsResponse`.

**Context:**
- File: `android/remoteapi/.../response/SyncResponses.kt` or similar

#### [x] 5.3.4 Implement API calls in RemoteApiClientImpl

**Description:** Implement the actual HTTP calls.

**Context:**
- File: `android/remoteapi/.../internal/RemoteApiClientImpl.kt`

---

### 5.4 Sync Manager

#### [x] 5.4.1 Create `SyncManager` interface

**Description:** Interface for sync operations.

**Context:**
- New file: `android/domain/src/main/java/com/lelloman/pezzottify/android/domain/sync/SyncManager.kt`

#### [x] 5.4.2 Create `SyncState` sealed interface

**Description:** Define sync state variants (Idle, Syncing, Synced, Error).

**Context:**
- File: `android/domain/.../sync/SyncManager.kt`

#### [x] 5.4.3 Implement `SyncManagerImpl`

**Description:** Implementation of sync manager with fullSync, catchUp, applyEvent.

**Context:**
- New file: `android/domain/src/main/java/.../sync/SyncManagerImpl.kt`

#### [x] 5.4.4 Add DI bindings for SyncManager

**Description:** Provide SyncManager via Hilt.

**Context:**
- File: `android/domain/.../DomainModule.kt` or similar

---

### 5.5 WebSocket Integration

#### [x] 5.5.1 Register sync message handler

**Description:** Handle incoming sync messages from WebSocket.

**Context:**
- File: `android/domain/.../sync/SyncWebSocketHandler.kt`
- Register handler for "sync" prefix

---

### 5.6 User Content Updates

#### [x] 5.6.1 Add apply methods to user content store

**Description:** Methods to apply individual sync events.

**Context:**
- File: `android/domain/.../sync/SyncManagerImpl.kt`
- SyncManagerImpl.applyStoredEvent() handles individual events via UserContentStore.setLiked()

#### [x] 5.6.2 Add setter methods for full sync

**Description:** Methods to set full state from sync.

**Context:**
- File: `android/domain/.../sync/SyncManagerImpl.kt`
- SyncManagerImpl.applyLikesState() handles full sync via UserContentStore.setLiked()

---

### 5.7 Auth Integration

#### [x] 5.7.1 Initialize sync on login

**Description:** Call syncManager.initialize() after successful login.

**Context:**
- File: `android/domain/.../auth/usecase/PerformLogin.kt`

#### [x] 5.7.2 Cleanup sync on logout

**Description:** Call syncManager.cleanup() before clearing auth.

**Context:**
- File: `android/domain/.../auth/usecase/PerformLogout.kt`

---

### 5.8 Testing

#### [x] 5.8.1 Write unit tests for SyncManagerImpl

**Description:** Test fullSync, catchUp, applyEvent logic.

**Context:**
- New file: `android/domain/src/test/java/.../sync/SyncManagerImplTest.kt`

#### [x] 5.8.2 Write unit tests for event serialization

**Description:** Test Kotlin event classes serialize/deserialize correctly.

**Context:**
- New file: `android/domain/src/test/java/.../sync/SyncEventTest.kt`

---

## Phase 6: Edge Cases

### 6.1 Sequence Gap Detection

#### [x] 6.1.1 Add gap detection to web catchUp

**Description:** Detect gaps and trigger fullSync.

**Context:**
- File: `web/src/store/sync.js`
- In `catchUp()`, check if first event seq > cursor + 1

#### [x] 6.1.2 Add gap detection to web WebSocket handler

**Description:** Detect gaps in real-time events.

**Context:**
- File: `web/src/store/sync.js`
- In message handler, check if event.seq > cursor + 1

#### [x] 6.1.3 Add gap detection to Android SyncManager

**Description:** Detect gaps and trigger fullSync.

**Context:**
- File: `android/domain/.../sync/SyncManagerImpl.kt`

---

## Phase 7: Event Log Maintenance

### 7.1 Background Pruning

#### [x] 7.1.1 Add CLI args for pruning configuration

**Description:** Add `--event-retention-days` and `--prune-interval-hours` args.

**Context:**
- File: `catalog-server/src/main.rs`
- Add to CLI argument parsing

#### [x] 7.1.2 Implement background pruning task

**Description:** Spawn async task that prunes periodically.

**Context:**
- File: `catalog-server/src/main.rs` or server setup

**Sample:**
```rust
let retention_days = args.event_retention_days.unwrap_or(30);
let interval_hours = args.prune_interval_hours.unwrap_or(24);

tokio::spawn(async move {
    let mut interval = tokio::time::interval(
        Duration::from_secs(interval_hours * 60 * 60)
    );
    loop {
        interval.tick().await;
        let cutoff = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64 - (retention_days * 24 * 60 * 60);

        match user_store.prune_events_older_than(cutoff) {
            Ok(count) => tracing::info!("Pruned {} old sync events", count),
            Err(e) => tracing::error!("Failed to prune sync events: {}", e),
        }
    }
});
```

---

## Phase 8: Testing

### 8.1 Server Tests

#### [x] 8.1.1 Integration test: sync state endpoint

**Description:** Test `GET /v1/sync/state` returns correct data.

#### [x] 8.1.2 Integration test: sync events endpoint

**Description:** Test `GET /v1/sync/events` returns events in order.

#### [x] 8.1.3 Integration test: 410 for pruned sequences

**Description:** Test that pruned sequences return 410.

#### [x] 8.1.4 Integration test: event generation

**Description:** Test that actions generate correct events.

#### [x] 8.1.5 Integration test: WebSocket broadcast

**Description:** Test that events are broadcast to other devices.

---

### 8.2 End-to-End Tests

#### [ ] 8.2.1 E2E test: fresh login full sync

**Description:** Test that first login does full sync.

#### [ ] 8.2.2 E2E test: page refresh catch-up

**Description:** Test that page refresh catches up on events.

#### [ ] 8.2.3 E2E test: two tabs real-time sync

**Description:** Test that action in one tab appears in another.

#### [ ] 8.2.4 E2E test: offline/reconnect

**Description:** Test that reconnecting after offline catches up.

#### [ ] 8.2.5 E2E test: web and Android sync

**Description:** Test that web and Android devices sync in real-time.

---

## Summary

| Phase | Tasks | Status |
|-------|-------|--------|
| 1. Event Log Infrastructure | 24 | Not started |
| 2. Sync API Endpoints | 9 | Not started |
| 3. WebSocket Push | 12 | Not started |
| 4. Web Frontend | 23 | Not started |
| 5. Android | 18 | Not started |
| 6. Edge Cases | 3 | Not started |
| 7. Maintenance | 2 | Not started |
| 8. Testing | 10 | Not started |
| **Total** | **101** | |
