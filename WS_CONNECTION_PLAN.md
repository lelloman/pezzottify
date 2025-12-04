# WebSocket Sync Feature Plan

## Prerequisites

This plan assumes the Device Entity feature has already been implemented:
- `device` table exists
- `auth_token` has `device_id` column (required)
- `Session` includes `device_id` and `device_type`
- Login requires device info

---

## Overview

Add real-time WebSocket sync to notify other devices when user data changes, with dirty flags for offline sessions.

## Key Design Decisions

- **Dirty flags tied to auth_token**: Logout clears dirty state (token deleted → dirty flags cascade deleted); new login triggers full data download anyway
- **Changelog-based WS messages**: Real-time notifications describe the specific change (e.g., "album X liked"), not full data
- **Category-based dirty flags**: Separate flags for `liked_content`, `playlists`, `user_permissions`, `user_settings`
- **Client-driven dirty clearing**: Client explicitly tells server when it has successfully synced

---

## Part 1: Dirty Flag System

### 1.1 Database Schema (Migration V8)

New table for tracking what data sessions (tokens) have missed:

```sql
CREATE TABLE token_dirty_flags (
    id INTEGER PRIMARY KEY,
    token_value TEXT NOT NULL REFERENCES auth_token(value) ON DELETE CASCADE,
    category TEXT NOT NULL,
    dirty_since INTEGER NOT NULL,
    UNIQUE (token_value, category)
);
CREATE INDEX idx_dirty_flags_token ON token_dirty_flags(token_value);
```

**Note**: Dirty flags are tied to auth tokens (active sessions), not devices. When a token is deleted (logout), its dirty flags are cascade deleted - this is intentional because a new login will trigger a full data fetch anyway.

### 1.2 Category Enum

**File: `catalog-server/src/user/sync.rs`** (new file)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncCategory {
    LikedContent,
    Playlists,
    UserPermissions,
    UserSettings,
}
```

### 1.3 Store Trait

**File: `catalog-server/src/user/user_store.rs`**

```rust
pub trait TokenDirtyFlagStore: Send + Sync {
    /// Mark category dirty for all OTHER tokens of this user
    fn mark_dirty_for_other_tokens(
        &self, user_id: usize, exclude_token: &AuthTokenValue, category: SyncCategory
    ) -> Result<()>;

    /// Mark category dirty for a specific token (when WS send fails)
    fn mark_dirty_for_token(&self, token: &AuthTokenValue, category: SyncCategory) -> Result<()>;

    /// Clear dirty flag (client confirms it has synced)
    fn clear_dirty_flag(&self, token: &AuthTokenValue, category: SyncCategory) -> Result<()>;

    /// Get all dirty categories for a token (called on WS connect)
    fn get_dirty_categories(&self, token: &AuthTokenValue) -> Result<Vec<SyncCategory>>;
}
```

---

## Part 2: WebSocket Infrastructure

### 2.1 New Module Structure

```
catalog-server/src/server/websocket/
├── mod.rs           # Module exports
├── messages.rs      # Message type definitions
├── connection.rs    # Connection manager
└── handlers.rs      # WS route handler
```

### 2.2 Message Protocol

**File: `catalog-server/src/server/websocket/messages.rs`**

```rust
/// Server -> Client messages
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
#[serde(rename_all = "snake_case")]
pub enum ServerMessage {
    /// Sent on connect with list of dirty categories
    Connected { dirty_categories: Vec<SyncCategory> },

    /// Liked content changed
    LikedContentChanged { content_type: String, content_id: String, liked: bool },

    /// Playlist changes
    PlaylistCreated { playlist_id: String, name: String },
    PlaylistUpdated { playlist_id: String, name: Option<String>, tracks_added: Option<Vec<String>>, tracks_removed_positions: Option<Vec<usize>> },
    PlaylistDeleted { playlist_id: String },

    /// Permissions changed by admin
    PermissionsChanged { permissions: Vec<String> },

    /// User setting changed
    SettingChanged { key: String, value: serde_json::Value },

    /// Heartbeat
    Pong,
}

/// Client -> Server messages
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
#[serde(rename_all = "snake_case")]
pub enum ClientMessage {
    Ping,
    ClearDirtyFlag { category: SyncCategory },
}
```

### 2.3 Connection Manager

**File: `catalog-server/src/server/websocket/connection.rs`**

Tracks active WebSocket connections per user:

```rust
pub struct ConnectionManager {
    /// user_id -> (token_value -> mpsc::Sender<ServerMessage>)
    connections: RwLock<HashMap<usize, HashMap<String, mpsc::Sender<ServerMessage>>>>,
}

impl ConnectionManager {
    /// Register a new connection
    pub async fn register(&self, user_id: usize, token: String, sender: mpsc::Sender<ServerMessage>);

    /// Unregister on disconnect
    pub async fn unregister(&self, user_id: usize, token: &str);

    /// Send to all OTHER devices of user (returns list of tokens that failed)
    pub async fn notify_other_devices(
        &self, user_id: usize, exclude_token: &str, message: ServerMessage
    ) -> Vec<String>;

    /// Send to ALL devices of user (for admin-triggered events)
    pub async fn broadcast_to_user(&self, user_id: usize, message: ServerMessage) -> Vec<String>;
}
```

### 2.4 WebSocket Route

**Endpoint**: `GET /v1/ws/sync`

Authentication via cookie or Authorization header (same as REST endpoints).

```rust
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    session: Session,
    State(state): State<ServerState>,
) -> Response {
    ws.on_upgrade(|socket| handle_socket(socket, session, state))
}

async fn handle_socket(socket: WebSocket, session: Session, state: ServerState) {
    // 1. Get dirty categories from DB
    // 2. Register connection with ConnectionManager
    // 3. Send Connected message with dirty categories
    // 4. Loop: receive client messages, send server messages
    // 5. On disconnect: unregister from ConnectionManager
}
```

### 2.5 ServerState Changes

**File: `catalog-server/src/server/state.rs`**

```rust
pub struct ServerState {
    // ... existing fields ...
    pub ws_connection_manager: Arc<ConnectionManager>,
}
```

---

## Part 3: Integration Points

### 3.1 Notification Helper

**File: `catalog-server/src/server/websocket/notify.rs`**

```rust
pub struct SyncNotifier {
    conn_manager: Arc<ConnectionManager>,
    user_store: Arc<Mutex<dyn FullUserStore>>,
}

impl SyncNotifier {
    /// Call after liked content changes
    pub async fn notify_liked_content_changed(
        &self, user_id: usize, source_token: &str,
        content_type: &str, content_id: &str, liked: bool
    ) {
        let message = ServerMessage::LikedContentChanged { ... };
        let failed = self.conn_manager.notify_other_devices(user_id, source_token, message).await;

        // Mark dirty for tokens that failed (offline)
        for token in failed {
            self.user_store.mark_dirty_for_token(&AuthTokenValue(token), SyncCategory::LikedContent);
        }

        // Also mark dirty for tokens not currently connected
        self.user_store.mark_dirty_for_other_tokens(user_id, source_token, SyncCategory::LikedContent);
    }

    // Similar methods for playlists, permissions, settings
}
```

### 3.2 Modified Handlers

Add notification calls to handlers that modify user data:

**Liked content** (`POST/DELETE /v1/user/liked/{type}/{id}`):
```rust
// After successful operation:
notifier.notify_liked_content_changed(session.user_id, &session.token, ...).await;
```

**Playlists** (`POST/PUT/DELETE /v1/user/playlist/*`):
```rust
notifier.notify_playlist_created/updated/deleted(session.user_id, &session.token, ...).await;
```

**Admin permission changes** (`PUT /v1/admin/user/{id}/permissions`):
```rust
// Use broadcast_to_user since this affects the target user, not the admin
notifier.broadcast_permissions_changed(target_user_id, new_permissions).await;
```

**User settings** (`PUT /v1/user/settings/{key}`):
```rust
notifier.notify_setting_changed(session.user_id, &session.token, key, value).await;
```

### 3.3 Clearing Dirty Flags

Client sends `ClearDirtyFlag` message via WS after it has successfully fetched AND processed/persisted the data. This is the only reliable approach since the server cannot know if the client successfully handled the fetched data (e.g., client could crash after fetch but before local persistence).

---

## Implementation Phases

### Phase 1: Dirty Flag Infrastructure
1. Create `sync.rs` with `SyncCategory` enum
2. Create `token_dirty_flags` table (migration V8)
3. Define `TokenDirtyFlagStore` trait in `user_store.rs`
4. Implement trait in `SqliteUserStore`
5. Add wrapper methods to `UserManager`

### Phase 2: WebSocket Module
1. Create `websocket/` module structure
2. Implement message types
3. Implement `ConnectionManager`
4. Implement WS handler
5. Add `ws_connection_manager` to `ServerState`
6. Register route `/v1/ws/sync`

### Phase 3: Handler Integration
1. Create `SyncNotifier` helper
2. Add notifier to `ServerState`
3. Modify liked content handlers
4. Modify playlist handlers
5. Modify admin permission handlers
6. Modify user settings handlers

---

## Critical Files

**Must modify:**
- `catalog-server/src/user/user_store.rs` - TokenDirtyFlagStore trait
- `catalog-server/src/user/sqlite_user_store.rs` - Migration V8, DirtyFlag implementation
- `catalog-server/src/user/user_manager.rs` - Wrapper methods
- `catalog-server/src/user/mod.rs` - Export sync module
- `catalog-server/src/server/server.rs` - Handler integration, WS route
- `catalog-server/src/server/state.rs` - Add ConnectionManager
- `catalog-server/src/server/mod.rs` - Export websocket module

**Must create:**
- `catalog-server/src/user/sync.rs` - SyncCategory enum
- `catalog-server/src/server/websocket/mod.rs`
- `catalog-server/src/server/websocket/messages.rs`
- `catalog-server/src/server/websocket/connection.rs`
- `catalog-server/src/server/websocket/handlers.rs`
- `catalog-server/src/server/websocket/notify.rs`

---

## Testing Strategy

1. **Unit tests** for `ConnectionManager` (register/unregister/notify)
2. **Unit tests** for dirty flag store operations
3. **Integration tests** for WS connect/disconnect
4. **Integration tests** for end-to-end sync flow:
   - Device A likes album → Device B receives WS notification
   - Device B offline → dirty flag set → Device B reconnects → sees dirty flag → fetches data → sends ClearDirtyFlag → dirty cleared

---

## Notes

- The WebSocket endpoint uses the same authentication as REST (cookie/header token)
- No new Cargo dependencies required - Axum 0.8 includes WS, tokio has `mpsc`
- Rate limiting not applied to WS upgrade itself (only to initial auth)
- Connection cleanup happens automatically via `Drop` when WS disconnects
