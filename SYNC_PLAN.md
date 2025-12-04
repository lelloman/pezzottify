# Multi-Device Sync Implementation Plan

## Overview

This document outlines the implementation plan for multi-device synchronization of user data in Pezzottify.

### Scope

The following user data will be synchronized:
- **Liked content** — albums, artists, tracks
- **User settings** — preferences like direct downloads
- **Playlists** — user-created playlists and their contents
- **Permissions** — granted by admins, affects UI/feature availability

### Goals

- When a user likes content on Device A, Device B sees it instantly (if online)
- When Device B comes back online after being offline, it catches up on missed changes
- When an admin grants/revokes permissions, user's devices update accordingly
- Simple, pragmatic approach — last-write-wins for conflicts

### Architecture

**Core concept:** Server maintains an append-only event log per user. Clients track their position (cursor) and sync via:
- REST endpoints for catch-up
- Real-time push (SSE) for instant updates

**Sync flows:**

1. **Online action:** Client → REST API → Server logs event → broadcasts to other devices
2. **Receiving push:** Server pushes event → Client applies to local state → updates cursor
3. **Reconnection:** Client fetches events since cursor → applies all → updates cursor
4. **Cursor too old:** Server returns 410 → Client does full state reset

---

## Phase 1: Server — Event Log Infrastructure

### 1.1 Database Schema

**New table: `user_events`**

```sql
CREATE TABLE user_events (
  seq INTEGER PRIMARY KEY AUTOINCREMENT,
  user_id INTEGER NOT NULL REFERENCES user(id) ON DELETE CASCADE,
  event_type TEXT NOT NULL,
  payload TEXT NOT NULL,        -- JSON
  client_timestamp INTEGER NOT NULL,
  server_timestamp INTEGER DEFAULT (cast(strftime('%s','now') as int))
);

CREATE INDEX idx_user_events_user_seq ON user_events(user_id, seq);
```

**Files to modify:**
- `catalog-server/src/user/sqlite_user_store.rs` — add schema version, migration

### 1.2 Event Types

**New file: `catalog-server/src/user/sync_events.rs`**

```rust
use serde::{Deserialize, Serialize};
use crate::user::user_models::ContentType;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum UserEvent {
    // Likes
    #[serde(rename = "content_liked")]
    ContentLiked {
        content_type: ContentType,
        content_id: String,
    },

    #[serde(rename = "content_unliked")]
    ContentUnliked {
        content_type: ContentType,
        content_id: String,
    },

    // Settings
    #[serde(rename = "setting_changed")]
    SettingChanged {
        key: String,
        value: serde_json::Value,
    },

    // Playlists
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

    #[serde(rename = "playlist_track_added")]
    PlaylistTrackAdded {
        playlist_id: String,
        track_id: String,
        position: u32,
    },

    #[serde(rename = "playlist_track_removed")]
    PlaylistTrackRemoved {
        playlist_id: String,
        track_id: String,
    },

    #[serde(rename = "playlist_tracks_reordered")]
    PlaylistTracksReordered {
        playlist_id: String,
        track_ids: Vec<String>,  // New order of all track IDs
    },

    // Permissions (triggered by admin actions)
    #[serde(rename = "permission_granted")]
    PermissionGranted {
        permission: String,
    },

    #[serde(rename = "permission_revoked")]
    PermissionRevoked {
        permission: String,
    },

    #[serde(rename = "permissions_reset")]
    PermissionsReset {
        permissions: Vec<String>,  // Full list of current permissions
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredEvent {
    pub seq: i64,
    pub event: UserEvent,
    pub client_timestamp: i64,
    pub server_timestamp: i64,
}
```

### 1.3 Event Log Storage

**Modify:** `catalog-server/src/user/sqlite_user_store.rs`

New methods:

```rust
/// Append an event to the user's event log
/// Returns the sequence number of the new event
pub fn append_event(
    &self,
    user_id: i64,
    event: &UserEvent,
    client_timestamp: i64,
) -> Result<i64, UserStoreError>;

/// Get events since a given sequence number
/// Returns Err(EventsPruned) if the sequence is too old
pub fn get_events_since(
    &self,
    user_id: i64,
    since_seq: i64,
) -> Result<Vec<StoredEvent>, UserStoreError>;

/// Get the current (latest) sequence number for a user
/// Returns 0 if no events exist
pub fn get_current_seq(&self, user_id: i64) -> Result<i64, UserStoreError>;

/// Delete events older than the given timestamp
/// Used for maintenance/pruning
pub fn prune_events_older_than(&self, timestamp: i64) -> Result<u64, UserStoreError>;
```

### 1.4 Integrate Event Logging into Existing Endpoints

**Modify:** `catalog-server/src/server/server.rs`

Update these handlers to append events after successful operations:

| Endpoint | Event |
|----------|-------|
| `POST /v1/user/liked/{type}/{id}` | `ContentLiked` |
| `DELETE /v1/user/liked/{type}/{id}` | `ContentUnliked` |
| `PUT /v1/user/settings` | `SettingChanged` (one per changed setting) |
| `POST /v1/user/playlists` | `PlaylistCreated` |
| `PUT /v1/user/playlists/{id}` | `PlaylistRenamed` |
| `DELETE /v1/user/playlists/{id}` | `PlaylistDeleted` |
| `POST /v1/user/playlists/{id}/tracks` | `PlaylistTrackAdded` |
| `DELETE /v1/user/playlists/{id}/tracks/{track_id}` | `PlaylistTrackRemoved` |
| `PUT /v1/user/playlists/{id}/tracks` | `PlaylistTracksReordered` |

**Permission events** are triggered by admin endpoints (when managing other users):

| Endpoint | Event (on target user's log) |
|----------|------------------------------|
| `POST /v1/admin/users/{id}/permissions` | `PermissionGranted` |
| `DELETE /v1/admin/users/{id}/permissions/{perm}` | `PermissionRevoked` |
| `PUT /v1/admin/users/{id}/role` | `PermissionsReset` |

Each handler will:
1. Perform the existing operation
2. Append event to log
3. Broadcast to connected clients (Phase 3)
4. Return response

---

## Phase 2: Server — Sync API Endpoints

### 2.1 Full State Endpoint

**New route:** `GET /v1/sync/state`

**Permission:** `AccessCatalog`

**Response:**
```json
{
  "seq": 42,
  "likes": {
    "albums": ["album_id_1", "album_id_2"],
    "artists": ["artist_id_1"],
    "tracks": ["track_id_1", "track_id_2"]
  },
  "settings": [
    { "key": "enable_direct_downloads", "value": true }
  ],
  "playlists": [
    {
      "id": "playlist_abc",
      "name": "Chill Vibes",
      "tracks": ["track_1", "track_2", "track_3"]
    }
  ],
  "permissions": [
    "access_catalog",
    "like_content",
    "own_playlists"
  ]
}
```

**Implementation notes:**
- Combines data from existing `get_user_liked_content()`, `get_all_user_settings()`, `get_user_playlists()`, and user permissions
- Returns current seq from `get_current_seq()`

### 2.2 Events Since Endpoint

**New route:** `GET /v1/sync/events?since={seq}`

**Permission:** `AccessCatalog`

**Query parameters:**
- `since` (required): sequence number to fetch events after

**Response (success — 200):**
```json
{
  "events": [
    {
      "seq": 43,
      "type": "content_liked",
      "payload": {
        "content_type": "album",
        "content_id": "album_123"
      },
      "timestamp": 1701700000
    },
    {
      "seq": 44,
      "type": "setting_changed",
      "payload": {
        "key": "enable_direct_downloads",
        "value": false
      },
      "timestamp": 1701700005
    }
  ],
  "current_seq": 44
}
```

**Response (pruned — 410 Gone):**
```json
{
  "error": "events_pruned",
  "message": "Requested sequence is no longer available. Please perform a full sync."
}
```

---

## Phase 3: Server — Real-Time Push

### 3.1 Connection Management

**New file:** `catalog-server/src/server/sync_broadcast.rs`

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use crate::user::sync_events::StoredEvent;

pub struct SyncBroadcaster {
    /// Map of user_id -> broadcast channel sender
    channels: RwLock<HashMap<i64, broadcast::Sender<StoredEvent>>>,
}

impl SyncBroadcaster {
    pub fn new() -> Self;

    /// Subscribe to events for a user
    /// Returns a receiver that will get all future events
    pub async fn subscribe(&self, user_id: i64) -> broadcast::Receiver<StoredEvent>;

    /// Broadcast an event to all connected clients for a user
    pub async fn broadcast(&self, user_id: i64, event: StoredEvent);

    /// Clean up channel when no subscribers remain
    async fn cleanup_if_empty(&self, user_id: i64);
}
```

### 3.2 SSE Push Endpoint

**New route:** `GET /v1/sync/stream`

**Permission:** `AccessCatalog`

**Response:** `Content-Type: text/event-stream`

**Event format:**
```
event: sync
data: {"seq":43,"type":"content_liked","payload":{"content_type":"album","content_id":"album_123"},"timestamp":1701700000}

event: sync
data: {"seq":44,"type":"setting_changed","payload":{"key":"enable_direct_downloads","value":false},"timestamp":1701700005}

```

**Implementation using Axum SSE:**

```rust
use axum::response::sse::{Event, Sse};
use futures::stream::Stream;

async fn sync_stream(
    session: Session,
    State(broadcaster): State<Arc<SyncBroadcaster>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let receiver = broadcaster.subscribe(session.user_id).await;

    let stream = BroadcastStream::new(receiver)
        .map(|result| {
            let event = result.unwrap();
            Ok(Event::default()
                .event("sync")
                .data(serde_json::to_string(&event).unwrap()))
        });

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(30))
    )
}
```

### 3.3 Broadcast on State Change

**Modify:** handlers in `server.rs`

After successful like/unlike/setting change:

```rust
// In add_user_liked_content handler:
let event = UserEvent::ContentLiked {
    content_type,
    content_id: content_id.clone(),
};
let seq = user_store.append_event(user_id, &event, client_timestamp)?;
let stored_event = StoredEvent { seq, event, client_timestamp, server_timestamp };
broadcaster.broadcast(user_id, stored_event).await;
```

---

## Phase 4: Web Frontend — Sync Client

### 4.1 Sync Store

**New file:** `web/src/store/sync.js`

```javascript
import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import { useUserStore } from './user';
import { useRemoteStore } from './remote';

export const useSyncStore = defineStore('sync', () => {
  const userStore = useUserStore();
  const remoteStore = useRemoteStore();

  // State
  const cursor = ref(null);          // Last seen sequence number
  const connected = ref(false);      // SSE connection status
  const eventSource = ref(null);     // SSE EventSource instance

  // Cursor persistence key (includes user ID)
  const cursorKey = computed(() => `sync_cursor_${userStore.userId}`);

  // Load cursor from localStorage
  function loadCursor() {
    const stored = localStorage.getItem(cursorKey.value);
    cursor.value = stored ? parseInt(stored, 10) : null;
  }

  // Save cursor to localStorage
  function saveCursor() {
    if (cursor.value !== null) {
      localStorage.setItem(cursorKey.value, cursor.value.toString());
    }
  }

  // Clear cursor (on logout)
  function clearCursor() {
    cursor.value = null;
    localStorage.removeItem(cursorKey.value);
  }

  // Full state sync
  async function fullSync() {
    const response = await remoteStore.fetchSyncState();

    // Update user store with full state
    userStore.setLikedAlbums(response.likes.albums);
    userStore.setLikedArtists(response.likes.artists);
    userStore.setLikedTracks(response.likes.tracks);
    userStore.setAllSettings(response.settings);
    userStore.setPlaylists(response.playlists);
    userStore.setPermissions(response.permissions);

    // Update cursor
    cursor.value = response.seq;
    saveCursor();
  }

  // Catch up on missed events
  async function catchUp() {
    if (cursor.value === null) {
      await fullSync();
      return;
    }

    try {
      const response = await remoteStore.fetchSyncEvents(cursor.value);

      // Apply each event
      for (const event of response.events) {
        applyEvent(event);
      }

      // Update cursor
      cursor.value = response.current_seq;
      saveCursor();
    } catch (error) {
      if (error.response?.status === 410) {
        // Events pruned, need full sync
        await fullSync();
      } else {
        throw error;
      }
    }
  }

  // Apply a single event to local state
  function applyEvent(event) {
    switch (event.type) {
      // Likes
      case 'content_liked':
        userStore.applyContentLiked(event.payload.content_type, event.payload.content_id);
        break;
      case 'content_unliked':
        userStore.applyContentUnliked(event.payload.content_type, event.payload.content_id);
        break;

      // Settings
      case 'setting_changed':
        userStore.applySettingChanged(event.payload.key, event.payload.value);
        break;

      // Playlists
      case 'playlist_created':
        userStore.applyPlaylistCreated(event.payload.playlist_id, event.payload.name);
        break;
      case 'playlist_renamed':
        userStore.applyPlaylistRenamed(event.payload.playlist_id, event.payload.name);
        break;
      case 'playlist_deleted':
        userStore.applyPlaylistDeleted(event.payload.playlist_id);
        break;
      case 'playlist_track_added':
        userStore.applyPlaylistTrackAdded(event.payload.playlist_id, event.payload.track_id, event.payload.position);
        break;
      case 'playlist_track_removed':
        userStore.applyPlaylistTrackRemoved(event.payload.playlist_id, event.payload.track_id);
        break;
      case 'playlist_tracks_reordered':
        userStore.applyPlaylistTracksReordered(event.payload.playlist_id, event.payload.track_ids);
        break;

      // Permissions
      case 'permission_granted':
        userStore.applyPermissionGranted(event.payload.permission);
        break;
      case 'permission_revoked':
        userStore.applyPermissionRevoked(event.payload.permission);
        break;
      case 'permissions_reset':
        userStore.applyPermissionsReset(event.payload.permissions);
        break;
    }

    // Update cursor
    cursor.value = event.seq;
    saveCursor();
  }

  // Connect to SSE stream
  function connect() {
    if (eventSource.value) {
      return; // Already connected
    }

    const url = `${remoteStore.baseUrl}/v1/sync/stream`;
    eventSource.value = new EventSource(url, { withCredentials: true });

    eventSource.value.addEventListener('sync', (e) => {
      const event = JSON.parse(e.data);
      applyEvent(event);
    });

    eventSource.value.onopen = () => {
      connected.value = true;
    };

    eventSource.value.onerror = async () => {
      connected.value = false;
      // SSE will auto-reconnect, but we need to catch up
      // Small delay to avoid hammering server
      setTimeout(async () => {
        if (eventSource.value?.readyState === EventSource.OPEN) {
          await catchUp();
        }
      }, 1000);
    };
  }

  // Disconnect from SSE stream
  function disconnect() {
    if (eventSource.value) {
      eventSource.value.close();
      eventSource.value = null;
      connected.value = false;
    }
  }

  // Initialize sync (called on app start / login)
  async function initialize() {
    loadCursor();
    await catchUp();  // Will do fullSync if no cursor
    connect();
  }

  // Cleanup (called on logout)
  function cleanup() {
    disconnect();
    clearCursor();
  }

  return {
    cursor,
    connected,
    initialize,
    cleanup,
    fullSync,
    catchUp,
    connect,
    disconnect,
  };
});
```

### 4.2 Event Application Methods

**Modify:** `web/src/store/user.js`

Add methods for applying sync events (update state without API calls):

```javascript
// Apply a like event from sync
function applyContentLiked(contentType, contentId) {
  switch (contentType) {
    case 'album':
      if (!likedAlbumIds.value.includes(contentId)) {
        likedAlbumIds.value.push(contentId);
      }
      break;
    case 'artist':
      if (!likedArtistsIds.value.includes(contentId)) {
        likedArtistsIds.value.push(contentId);
      }
      break;
    case 'track':
      if (!likedTrackIds.value.includes(contentId)) {
        likedTrackIds.value.push(contentId);
      }
      break;
  }
}

// Apply an unlike event from sync
function applyContentUnliked(contentType, contentId) {
  switch (contentType) {
    case 'album':
      likedAlbumIds.value = likedAlbumIds.value.filter(id => id !== contentId);
      break;
    case 'artist':
      likedArtistsIds.value = likedArtistsIds.value.filter(id => id !== contentId);
      break;
    case 'track':
      likedTrackIds.value = likedTrackIds.value.filter(id => id !== contentId);
      break;
  }
}

// Apply a setting change event from sync
function applySettingChanged(key, value) {
  settings.value[key] = value;
}

// Setters for full state (used by fullSync)
function setLikedAlbums(ids) {
  likedAlbumIds.value = ids;
}

function setLikedArtists(ids) {
  likedArtistsIds.value = ids;
}

function setLikedTracks(ids) {
  likedTrackIds.value = ids;
}

function setAllSettings(settingsArray) {
  settings.value = {};
  for (const setting of settingsArray) {
    settings.value[setting.key] = setting.value;
  }
}

// --- Playlist event handlers ---

function applyPlaylistCreated(playlistId, name) {
  playlists.value.push({
    id: playlistId,
    name: name,
    tracks: []
  });
}

function applyPlaylistRenamed(playlistId, name) {
  const playlist = playlists.value.find(p => p.id === playlistId);
  if (playlist) {
    playlist.name = name;
  }
}

function applyPlaylistDeleted(playlistId) {
  playlists.value = playlists.value.filter(p => p.id !== playlistId);
}

function applyPlaylistTrackAdded(playlistId, trackId, position) {
  const playlist = playlists.value.find(p => p.id === playlistId);
  if (playlist) {
    playlist.tracks.splice(position, 0, trackId);
  }
}

function applyPlaylistTrackRemoved(playlistId, trackId) {
  const playlist = playlists.value.find(p => p.id === playlistId);
  if (playlist) {
    playlist.tracks = playlist.tracks.filter(id => id !== trackId);
  }
}

function applyPlaylistTracksReordered(playlistId, trackIds) {
  const playlist = playlists.value.find(p => p.id === playlistId);
  if (playlist) {
    playlist.tracks = trackIds;
  }
}

function setPlaylists(playlistsData) {
  playlists.value = playlistsData;
}

// --- Permission event handlers ---

function applyPermissionGranted(permission) {
  if (!permissions.value.includes(permission)) {
    permissions.value.push(permission);
  }
}

function applyPermissionRevoked(permission) {
  permissions.value = permissions.value.filter(p => p !== permission);
}

function applyPermissionsReset(newPermissions) {
  permissions.value = newPermissions;
}

function setPermissions(perms) {
  permissions.value = perms;
}
```

### 4.3 Remote Store Additions

**Modify:** `web/src/store/remote.js`

Add sync API methods:

```javascript
// Fetch full sync state
async function fetchSyncState() {
  const response = await axios.get(`${baseUrl}/v1/sync/state`);
  return response.data;
}

// Fetch events since sequence number
async function fetchSyncEvents(since) {
  const response = await axios.get(`${baseUrl}/v1/sync/events`, {
    params: { since }
  });
  return response.data;
}
```

### 4.4 Integration with App Lifecycle

**Modify:** `web/src/store/user.js` — `initialize()`

```javascript
async function initialize() {
  const syncStore = useSyncStore();
  await syncStore.initialize();
  // Remove existing fetch calls — sync handles this now
}
```

**Modify:** auth/logout flow

```javascript
function logout() {
  const syncStore = useSyncStore();
  syncStore.cleanup();
  // ... existing logout logic
}
```

---

## Phase 5: Cursor Persistence & Edge Cases

### 5.1 Cursor Storage

- **Key format:** `sync_cursor_{userId}`
- **Storage:** `localStorage`
- **Updated:** after each event application

### 5.2 Fresh Login

- No cursor in localStorage
- `catchUp()` sees `cursor === null`
- Calls `fullSync()`

### 5.3 Logout

- Call `syncStore.cleanup()`
- Clears cursor from localStorage
- Closes SSE connection

### 5.4 Multiple Tabs

**Initial approach:** each tab has its own SSE connection

- Simple to implement
- Slightly wasteful (multiple connections per user)
- No cross-tab sync issues

**Future optimization (optional):**
- Use `BroadcastChannel` API to share events between tabs
- Only one tab maintains SSE connection
- Complexity: leader election, tab close handling

### 5.5 Page Visibility

**Optional optimization:**

```javascript
document.addEventListener('visibilitychange', () => {
  if (document.visibilityState === 'visible') {
    // Tab became visible, catch up in case we missed events
    syncStore.catchUp();
  }
});
```

---

## Phase 6: Event Log Maintenance

### 6.1 Pruning Strategy

**Recommended:** Time-based pruning

- Keep events for last 30 days
- Simple to reason about
- Clients offline longer than 30 days do full sync

**Alternative:** Count-based

- Keep last N events per user (e.g., 1000)
- Bounded storage per user
- May prune recent events for very active users

### 6.2 Pruning Implementation

**Add to:** `catalog-server/src/user/sqlite_user_store.rs`

```rust
/// Delete events older than the given Unix timestamp
pub fn prune_events_older_than(&self, before_timestamp: i64) -> Result<u64, UserStoreError> {
    let deleted = self.conn.execute(
        "DELETE FROM user_events WHERE server_timestamp < ?",
        [before_timestamp],
    )?;
    Ok(deleted as u64)
}
```

### 6.3 Pruning Execution

**Options:**

1. **CLI command:** Run via cron job
   ```bash
   cargo run --bin cli-maintenance -- prune-events --older-than-days 30
   ```

2. **Background task:** Spawn async task on server startup that runs periodically

3. **On-demand:** Prune during low-traffic periods (e.g., triggered by admin endpoint)

**Recommended:** CLI command with cron for simplicity.

### 6.4 Detecting Pruned Sequences

When client requests `GET /v1/sync/events?since=N`:

```rust
// Check if we have any events at or before the requested sequence
let min_seq = get_min_seq_for_user(user_id)?;

if since_seq < min_seq {
    return Err(StatusCode::GONE); // 410
}

// Otherwise, return events
```

---

## File Summary

### New Files

| File | Description |
|------|-------------|
| `catalog-server/src/user/sync_events.rs` | Event type definitions |
| `catalog-server/src/server/sync_broadcast.rs` | Push connection management |
| `web/src/store/sync.js` | Sync client logic |

### Modified Files

| File | Changes |
|------|---------|
| `catalog-server/src/user/sqlite_user_store.rs` | Schema migration, event log methods |
| `catalog-server/src/user/mod.rs` | Export sync_events module |
| `catalog-server/src/server/server.rs` | New routes, broadcast integration |
| `catalog-server/src/server/mod.rs` | Export sync_broadcast module |
| `web/src/store/user.js` | Event application methods, init flow |
| `web/src/store/remote.js` | Sync API methods |

---

## Implementation Order

### Step 1: Database & Event Types
- Add `user_events` table schema
- Create `sync_events.rs` with event types
- Implement event log storage methods

### Step 2: Event Logging
- Modify like/unlike handlers to append events
- Modify settings handler to append events
- Modify playlist handlers to append events
- Modify admin permission handlers to append events (on target user's log)
- (No broadcast yet — just logging)

### Step 3: Sync REST Endpoints
- Implement `GET /v1/sync/state`
- Implement `GET /v1/sync/events?since=N`
- Handle 410 for pruned sequences

### Step 4: Web Catch-Up Sync
- Create `sync.js` store
- Implement `fullSync()` and `catchUp()`
- Integrate with user store
- Test: change data via API, refresh page, verify sync

### Step 5: SSE Push
- Implement `SyncBroadcaster`
- Add `GET /v1/sync/stream` endpoint
- Modify handlers to broadcast after logging

### Step 6: Web Real-Time
- Implement SSE connection in `sync.js`
- Test: open two browser tabs, like in one, see update in other

### Step 7: Pruning
- Implement `prune_events_older_than()`
- Create CLI command or background task
- Test: prune events, verify 410 response, verify full sync recovery

---

## Testing Checklist

### Unit Tests
- [ ] Event serialization/deserialization
- [ ] Event log append and retrieval
- [ ] Pruning logic
- [ ] 410 detection for pruned sequences

### Integration Tests
- [ ] `GET /v1/sync/state` returns correct data (likes, settings, playlists, permissions)
- [ ] `GET /v1/sync/events` returns events in order
- [ ] `GET /v1/sync/events` returns 410 for old sequences
- [ ] Like action generates event
- [ ] Settings change generates event
- [ ] Playlist CRUD generates events
- [ ] Admin permission changes generate events on target user's log

### End-to-End Tests
- [ ] Fresh login → full sync works
- [ ] Page refresh → catch-up sync works
- [ ] Two tabs → real-time sync via SSE
- [ ] Offline → reconnect → catch-up works
- [ ] Long offline → 410 → full sync recovery

---

## Future Enhancements

### Potential Additions
- **Offline queue:** Allow actions while offline, sync when back online
- **Compression:** Compact redundant events (like + unlike same item)
- **Selective sync:** Subscribe to specific event types
- **WebSocket:** Add as alternative to SSE for bidirectional needs

### Performance Optimizations
- **Tab coordination:** Use BroadcastChannel to share one SSE connection
- **Batching:** Batch multiple events into single SSE message
- **Debouncing:** Debounce rapid changes before broadcasting

---

## Appendix: Event Reference

### All Event Types

| Event Type | Triggered By | Payload |
|------------|--------------|---------|
| `content_liked` | User likes content | `{ content_type, content_id }` |
| `content_unliked` | User unlikes content | `{ content_type, content_id }` |
| `setting_changed` | User changes setting | `{ key, value }` |
| `playlist_created` | User creates playlist | `{ playlist_id, name }` |
| `playlist_renamed` | User renames playlist | `{ playlist_id, name }` |
| `playlist_deleted` | User deletes playlist | `{ playlist_id }` |
| `playlist_track_added` | User adds track to playlist | `{ playlist_id, track_id, position }` |
| `playlist_track_removed` | User removes track from playlist | `{ playlist_id, track_id }` |
| `playlist_tracks_reordered` | User reorders playlist | `{ playlist_id, track_ids }` |
| `permission_granted` | Admin grants permission | `{ permission }` |
| `permission_revoked` | Admin revokes permission | `{ permission }` |
| `permissions_reset` | Admin changes user role | `{ permissions }` |

### Permission Values

Permissions that can appear in events:
- `access_catalog`
- `like_content`
- `own_playlists`
- `edit_catalog`
- `manage_permissions`
- `issue_content_download`
- `reboot_server`
