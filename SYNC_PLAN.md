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
- Real-time push via existing WebSocket (`GET /v1/ws`)

**Sync flows:**

1. **Online action:** Client → REST API → Server logs event → broadcasts to other devices via WebSocket
2. **Receiving push:** Server pushes event → Client applies to local state → updates cursor
3. **Reconnection:** Client fetches events since cursor → applies all → updates cursor
4. **Cursor too old:** Server returns 410 → Client does full state reset
5. **Sequence gap:** Client detects gap → triggers full state reset

**Important:** The server broadcasts events only to *other* devices of the same user. The device that performed the action already applied it optimistically — no need to echo back.

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
use crate::user::permissions::Permission;
use crate::user::settings::UserSetting;

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
        setting: UserSetting,
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

    #[serde(rename = "playlist_tracks_updated")]
    PlaylistTracksUpdated {
        playlist_id: String,
        track_ids: Vec<String>,  // Full list of tracks (replaces previous)
    },

    // Permissions (triggered by CLI admin actions)
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
        permissions: Vec<Permission>,  // Full list of current permissions
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredEvent {
    pub seq: i64,
    pub event: UserEvent,
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

/// Get the minimum available sequence number for a user
/// Used to detect if requested sequence has been pruned
pub fn get_min_seq(&self, user_id: i64) -> Result<i64, UserStoreError>;

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
| `PUT /v1/user/settings` | `SettingChanged` |
| `POST /v1/user/playlist` | `PlaylistCreated` |
| `PUT /v1/user/playlist/{id}` | `PlaylistRenamed` and/or `PlaylistTracksUpdated` |
| `DELETE /v1/user/playlist/{id}` | `PlaylistDeleted` |
| `PUT /v1/user/playlist/{id}/add` | `PlaylistTracksUpdated` |
| `PUT /v1/user/playlist/{id}/remove` | `PlaylistTracksUpdated` |

**Permission events** are triggered by CLI admin commands. The CLI must call event logging directly:

| CLI Command | Event (on target user's log) |
|-------------|------------------------------|
| Grant permission | `PermissionGranted` |
| Revoke permission | `PermissionRevoked` |
| Change role | `PermissionsReset` |

Each handler will:
1. Perform the existing operation
2. Append event to log
3. Broadcast to other connected devices (Phase 3)
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
    "AccessCatalog",
    "LikeContent",
    "OwnPlaylists"
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
        "setting": {
          "key": "enable_direct_downloads",
          "value": false
        }
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

## Phase 3: Server — Real-Time Push via WebSocket

### 3.1 Extend Existing WebSocket Infrastructure

The server already has WebSocket support at `GET /v1/ws` with connection management and message routing. We'll extend it to handle sync events.

**Modify:** `catalog-server/src/server/websocket/messages.rs`

Add sync message types:

```rust
pub mod msg_types {
    // ... existing types ...
    pub const SYNC: &str = "sync";
}

pub mod sync {
    use serde::{Deserialize, Serialize};
    use crate::user::sync_events::StoredEvent;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct SyncEvent {
        pub event: StoredEvent,
    }
}
```

### 3.2 Broadcast to Other Devices

**Modify:** `catalog-server/src/server/websocket/connection.rs`

Add method to broadcast to all devices except one:

```rust
impl ConnectionManager {
    /// Broadcast a message to all connected devices for a user except the source device
    pub async fn broadcast_to_others(
        &self,
        user_id: usize,
        source_device_id: usize,
        message: ServerMessage,
    ) {
        // Get all device connections for user
        // Send to all except source_device_id
    }
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
let seq = user_store.append_event(user_id, &event)?;
let stored_event = StoredEvent { seq, event, server_timestamp };

// Broadcast to OTHER devices only (not the one making this request)
if let Some(device_id) = session.device_id {
    let ws_message = ServerMessage::new(msg_types::SYNC, sync::SyncEvent { event: stored_event });
    connection_manager.broadcast_to_others(user_id, device_id, ws_message).await;
}
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
  const connected = ref(false);      // WebSocket connection status
  const webSocket = ref(null);       // WebSocket instance

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

      // Check for sequence gaps
      if (response.events.length > 0) {
        const firstSeq = response.events[0].seq;
        if (firstSeq > cursor.value + 1) {
          // Gap detected, do full resync
          console.warn('Sequence gap detected, performing full sync');
          await fullSync();
          return;
        }
      }

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
        userStore.applySettingChanged(event.payload.setting);
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
      case 'playlist_tracks_updated':
        userStore.applyPlaylistTracksUpdated(event.payload.playlist_id, event.payload.track_ids);
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

  // Connect to WebSocket
  function connect() {
    if (webSocket.value) {
      return; // Already connected
    }

    const wsUrl = remoteStore.baseUrl.replace(/^http/, 'ws') + '/v1/ws';
    webSocket.value = new WebSocket(wsUrl);

    webSocket.value.onopen = () => {
      connected.value = true;
    };

    webSocket.value.onmessage = (e) => {
      const message = JSON.parse(e.data);
      if (message.type === 'sync') {
        applyEvent(message.payload.event);
      }
      // Handle other message types (connected, pong, etc.) as needed
    };

    webSocket.value.onclose = async () => {
      connected.value = false;
      webSocket.value = null;
      // Reconnect after delay
      setTimeout(() => {
        if (userStore.isInitialized) {
          catchUp().then(() => connect());
        }
      }, 1000);
    };

    webSocket.value.onerror = () => {
      connected.value = false;
    };
  }

  // Disconnect from WebSocket
  function disconnect() {
    if (webSocket.value) {
      webSocket.value.close();
      webSocket.value = null;
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

Add `likedTrackIds` state and methods for applying sync events:

```javascript
const likedTrackIds = ref(null);

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
function applySettingChanged(setting) {
  settings.value[setting.key] = setting.value;
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
  if (!playlistsData.value) return;
  const newPlaylist = { id: playlistId, name: name, tracks: [] };
  playlistsData.value.list.push(newPlaylist);
  playlistsData.value.by_id[playlistId] = newPlaylist;
}

function applyPlaylistRenamed(playlistId, name) {
  if (!playlistsData.value?.by_id[playlistId]) return;
  playlistsData.value.by_id[playlistId].name = name;
}

function applyPlaylistDeleted(playlistId) {
  if (!playlistsData.value) return;
  playlistsData.value.list = playlistsData.value.list.filter(p => p.id !== playlistId);
  delete playlistsData.value.by_id[playlistId];
}

function applyPlaylistTracksUpdated(playlistId, trackIds) {
  if (!playlistsData.value?.by_id[playlistId]) return;
  playlistsData.value.by_id[playlistId].tracks = trackIds;
}

function setPlaylists(playlistsList) {
  const by_id = {};
  playlistsList.forEach(playlist => {
    by_id[playlist.id] = playlist;
  });
  playlistsData.value = { list: playlistsList, by_id };
}

// --- Permission event handlers ---

const permissions = ref([]);

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

## Phase 5: Android — Sync Client

### 5.1 Sync Event Models

**New file:** `android/domain/src/main/java/com/lelloman/pezzottify/android/domain/sync/SyncEvent.kt`

```kotlin
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
sealed interface SyncEventPayload

@Serializable
@SerialName("content_liked")
data class ContentLiked(
    @SerialName("content_type") val contentType: String,
    @SerialName("content_id") val contentId: String
) : SyncEventPayload

@Serializable
@SerialName("content_unliked")
data class ContentUnliked(
    @SerialName("content_type") val contentType: String,
    @SerialName("content_id") val contentId: String
) : SyncEventPayload

@Serializable
@SerialName("setting_changed")
data class SettingChanged(
    val setting: UserSettingValue
) : SyncEventPayload

@Serializable
@SerialName("playlist_created")
data class PlaylistCreated(
    @SerialName("playlist_id") val playlistId: String,
    val name: String
) : SyncEventPayload

@Serializable
@SerialName("playlist_renamed")
data class PlaylistRenamed(
    @SerialName("playlist_id") val playlistId: String,
    val name: String
) : SyncEventPayload

@Serializable
@SerialName("playlist_deleted")
data class PlaylistDeleted(
    @SerialName("playlist_id") val playlistId: String
) : SyncEventPayload

@Serializable
@SerialName("playlist_tracks_updated")
data class PlaylistTracksUpdated(
    @SerialName("playlist_id") val playlistId: String,
    @SerialName("track_ids") val trackIds: List<String>
) : SyncEventPayload

@Serializable
@SerialName("permission_granted")
data class PermissionGranted(
    val permission: String
) : SyncEventPayload

@Serializable
@SerialName("permission_revoked")
data class PermissionRevoked(
    val permission: String
) : SyncEventPayload

@Serializable
@SerialName("permissions_reset")
data class PermissionsReset(
    val permissions: List<String>
) : SyncEventPayload

@Serializable
data class StoredEvent(
    val seq: Long,
    val event: SyncEventPayload,
    @SerialName("server_timestamp") val serverTimestamp: Long
)
```

### 5.2 Sync State Store

**New file:** `android/domain/src/main/java/com/lelloman/pezzottify/android/domain/sync/SyncStateStore.kt`

```kotlin
interface SyncStateStore {
    suspend fun getCursor(): Long?
    suspend fun setCursor(seq: Long)
    suspend fun clearCursor()
}
```

**Implementation in localdata module** using EncryptedSharedPreferences.

### 5.3 Sync API Client

**Modify:** `android/remoteapi/.../RemoteApiClient.kt`

Add sync endpoints:

```kotlin
interface RemoteApiClient {
    // ... existing methods ...

    suspend fun getSyncState(): SyncStateResponse
    suspend fun getSyncEvents(since: Long): SyncEventsResponse
}
```

### 5.4 Sync Manager

**New file:** `android/domain/src/main/java/com/lelloman/pezzottify/android/domain/sync/SyncManager.kt`

```kotlin
interface SyncManager {
    val syncState: StateFlow<SyncState>

    suspend fun initialize()
    suspend fun fullSync()
    suspend fun catchUp()
    fun cleanup()
}

sealed interface SyncState {
    data object Idle : SyncState
    data object Syncing : SyncState
    data class Synced(val seq: Long) : SyncState
    data class Error(val message: String) : SyncState
}
```

### 5.5 WebSocket Sync Handler

**Modify:** `android/domain/src/main/java/com/lelloman/pezzottify/android/domain/websocket/WebSocketInitializer.kt`

Register sync message handler:

```kotlin
webSocketManager.registerHandler("sync") { message ->
    val syncMessage = Json.decodeFromString<SyncMessage>(message.payload)
    syncManager.applyEvent(syncMessage.event)
}
```

### 5.6 User Content Store Updates

**Modify:** `android/domain/.../UserContentStore.kt` (or equivalent)

Add methods for applying sync events:

```kotlin
interface UserContentStore {
    // ... existing methods ...

    suspend fun applyContentLiked(contentType: String, contentId: String)
    suspend fun applyContentUnliked(contentType: String, contentId: String)
    suspend fun applySettingChanged(setting: UserSettingValue)
    suspend fun applyPlaylistCreated(playlistId: String, name: String)
    suspend fun applyPlaylistRenamed(playlistId: String, name: String)
    suspend fun applyPlaylistDeleted(playlistId: String)
    suspend fun applyPlaylistTracksUpdated(playlistId: String, trackIds: List<String>)

    // Full state setters for fullSync
    suspend fun setLikedContent(albums: List<String>, artists: List<String>, tracks: List<String>)
    suspend fun setPlaylists(playlists: List<Playlist>)
    suspend fun setPermissions(permissions: List<String>)
}
```

### 5.7 Integration with App Lifecycle

**Modify:** `android/domain/.../auth/usecase/PerformLogin.kt`

After successful login, initialize sync:

```kotlin
syncManager.initialize()
```

**Modify:** `android/domain/.../auth/usecase/PerformLogout.kt`

Before clearing auth, cleanup sync:

```kotlin
syncManager.cleanup()
```

---

## Phase 6: Cursor Persistence & Edge Cases

### 6.1 Cursor Storage

- **Key format:** `sync_cursor_{userId}`
- **Storage:**
  - Web: `localStorage`
  - Android: `EncryptedSharedPreferences`
- **Updated:** after each event application

### 6.2 Fresh Login

- No cursor in storage
- `catchUp()` sees `cursor === null`
- Calls `fullSync()`

### 6.3 Logout

- Call `syncManager.cleanup()` / `syncStore.cleanup()`
- Clears cursor from storage
- Closes WebSocket connection

### 6.4 Sequence Gap Detection

When receiving events via catch-up:
```javascript
if (response.events.length > 0) {
  const firstSeq = response.events[0].seq;
  if (firstSeq > cursor + 1) {
    // Gap detected, do full resync
    await fullSync();
    return;
  }
}
```

When receiving events via WebSocket:
```javascript
if (event.seq > cursor + 1) {
  // Gap detected, do full resync
  await fullSync();
  return;
}
```

### 6.5 Multiple Tabs (Web)

**Initial approach:** each tab has its own WebSocket connection

- Simple to implement
- Slightly wasteful (multiple connections per user)
- No cross-tab sync issues

**Future optimization (optional):**
- Use `BroadcastChannel` API to share events between tabs
- Only one tab maintains WebSocket connection
- Complexity: leader election, tab close handling

### 6.6 Page Visibility (Web)

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

## Phase 7: Event Log Maintenance

### 7.1 Pruning Strategy

**Recommended:** Time-based pruning

- Keep events for last 30 days
- Simple to reason about
- Clients offline longer than 30 days do full sync

**Alternative:** Count-based

- Keep last N events per user (e.g., 1000)
- Bounded storage per user
- May prune recent events for very active users

### 7.2 Pruning Implementation

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

### 7.3 Pruning Execution

**Approach:** Internal background task

Spawn an async task on server startup that runs periodically:

```rust
// In server startup
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(24 * 60 * 60)); // Daily
    loop {
        interval.tick().await;
        let cutoff = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64 - (30 * 24 * 60 * 60); // 30 days ago

        match user_store.prune_events_older_than(cutoff) {
            Ok(count) => tracing::info!("Pruned {} old sync events", count),
            Err(e) => tracing::error!("Failed to prune sync events: {}", e),
        }
    }
});
```

**Configuration:** Add optional CLI args:
- `--event-retention-days <DAYS>`: How long to keep events (default: 30)
- `--prune-interval-hours <HOURS>`: How often to run pruning (default: 24)

### 7.4 Detecting Pruned Sequences

When client requests `GET /v1/sync/events?since=N`:

```rust
// Check if we have any events at or before the requested sequence
// This is O(1) with the (user_id, seq) index
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
| `web/src/store/sync.js` | Web sync client logic |
| `android/domain/.../sync/SyncEvent.kt` | Android event models |
| `android/domain/.../sync/SyncManager.kt` | Android sync manager |
| `android/domain/.../sync/SyncStateStore.kt` | Android cursor persistence |

### Modified Files

| File | Changes |
|------|---------|
| `catalog-server/src/user/sqlite_user_store.rs` | Schema migration, event log methods |
| `catalog-server/src/user/mod.rs` | Export sync_events module |
| `catalog-server/src/server/server.rs` | New routes, broadcast integration |
| `catalog-server/src/server/websocket/messages.rs` | Sync message types |
| `catalog-server/src/server/websocket/connection.rs` | broadcast_to_others method |
| `catalog-server/src/bin/cli-auth.rs` | Event logging for permission changes |
| `catalog-server/src/main.rs` | Background pruning task |
| `web/src/store/user.js` | Event application methods, likedTrackIds, permissions |
| `web/src/store/remote.js` | Sync API methods |
| `android/remoteapi/.../RemoteApiClient.kt` | Sync API methods |
| `android/domain/.../websocket/WebSocketInitializer.kt` | Sync handler registration |

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
- Modify CLI to log permission events
- (No broadcast yet — just logging)

### Step 3: Sync REST Endpoints
- Implement `GET /v1/sync/state`
- Implement `GET /v1/sync/events?since=N`
- Handle 410 for pruned sequences

### Step 4: Web Catch-Up Sync
- Create `sync.js` store
- Add `likedTrackIds` to user store
- Implement `fullSync()` and `catchUp()`
- Integrate with user store
- Test: change data via API, refresh page, verify sync

### Step 5: WebSocket Push
- Add sync message types
- Add `broadcast_to_others()` method
- Modify handlers to broadcast after logging

### Step 6: Web Real-Time
- Implement WebSocket connection in `sync.js`
- Test: open two browser tabs, like in one, see update in other

### Step 7: Android Sync
- Create sync event models
- Implement SyncStateStore
- Add sync API methods
- Implement SyncManager
- Register WebSocket sync handler
- Add event application methods to stores
- Integrate with login/logout

### Step 8: Pruning
- Implement `prune_events_older_than()`
- Add background pruning task to server startup
- Add `--event-retention-days` and `--prune-interval-hours` CLI args
- Test: prune events, verify 410 response, verify full sync recovery

---

## Testing Checklist

### Unit Tests
- [ ] Event serialization/deserialization
- [ ] Event log append and retrieval
- [ ] Pruning logic
- [ ] 410 detection for pruned sequences
- [ ] Sequence gap detection

### Integration Tests
- [ ] `GET /v1/sync/state` returns correct data (likes, settings, playlists, permissions)
- [ ] `GET /v1/sync/events` returns events in order
- [ ] `GET /v1/sync/events` returns 410 for old sequences
- [ ] Like action generates event
- [ ] Settings change generates event
- [ ] Playlist CRUD generates events
- [ ] CLI permission changes generate events on target user's log
- [ ] WebSocket broadcast excludes source device

### End-to-End Tests
- [ ] Fresh login → full sync works
- [ ] Page refresh → catch-up sync works
- [ ] Two tabs → real-time sync via WebSocket
- [ ] Offline → reconnect → catch-up works
- [ ] Long offline → 410 → full sync recovery
- [ ] Sequence gap → full sync recovery
- [ ] Web and Android devices sync in real-time

---

## Future Enhancements

### Potential Additions
- **Offline queue:** Allow actions while offline, sync when back online
- **Compression:** Compact redundant events (like + unlike same item)
- **Selective sync:** Subscribe to specific event types

### Performance Optimizations
- **Tab coordination:** Use BroadcastChannel to share one WebSocket connection
- **Batching:** Batch multiple events into single WebSocket message
- **Debouncing:** Debounce rapid changes before broadcasting

---

## Appendix: Event Reference

### All Event Types

| Event Type | Triggered By | Payload |
|------------|--------------|---------|
| `content_liked` | User likes content | `{ content_type, content_id }` |
| `content_unliked` | User unlikes content | `{ content_type, content_id }` |
| `setting_changed` | User changes setting | `{ setting: UserSetting }` |
| `playlist_created` | User creates playlist | `{ playlist_id, name }` |
| `playlist_renamed` | User renames playlist | `{ playlist_id, name }` |
| `playlist_deleted` | User deletes playlist | `{ playlist_id }` |
| `playlist_tracks_updated` | User modifies playlist tracks | `{ playlist_id, track_ids }` |
| `permission_granted` | Admin grants permission (via CLI) | `{ permission: Permission }` |
| `permission_revoked` | Admin revokes permission (via CLI) | `{ permission: Permission }` |
| `permissions_reset` | Admin changes user role (via CLI) | `{ permissions: Vec<Permission> }` |

### Permission Values

Permissions that can appear in events (from `permissions.rs`):
- `AccessCatalog`
- `LikeContent`
- `OwnPlaylists`
- `EditCatalog`
- `ManagePermissions`
- `IssueContentDownload`
- `RebootServer`
- `ViewAnalytics`
