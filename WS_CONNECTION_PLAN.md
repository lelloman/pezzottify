# WebSocket Infrastructure Plan

## Prerequisites

This plan assumes the Device Entity feature has already been implemented:
- `device` table exists
- `auth_token` has `device_id` column (required)
- `Session` includes `device_id` and `device_type`
- Login requires device info

---

## Overview

Add a generic WebSocket infrastructure to support real-time bidirectional communication between server and clients. This foundation will enable features like:
- User data synchronization across devices
- Remote playback control
- Real-time notifications
- Admin broadcasts

This plan covers **only** the WebSocket infrastructure itself, not any specific feature built on top of it.

---

## Key Design Decisions

- **Authentication via existing session**: WS upgrade uses cookies (browsers) or Authorization header (native clients) - same as REST endpoints
- **Per-user connection registry**: Server tracks active connections grouped by user
- **Device-aware connections**: Each connection is tied to a specific device (from session)
- **Drop-and-replace on reconnect**: If a device reconnects while old connection exists, the old one is dropped silently
- **Generic message envelope**: Flexible message format that can be extended for various features
- **Graceful degradation**: Connection drops are expected; clients should reconnect automatically
- **Idle timeout**: Server drops connections that stop responding to WebSocket ping frames

---

## Part 1: WebSocket Module Structure

### 1.1 New Module Layout

```
catalog-server/src/server/websocket/
├── mod.rs           # Module exports
├── messages.rs      # Message type definitions (envelope + routing)
├── connection.rs    # Connection manager (registry of active connections)
└── handler.rs       # WS route handler (upgrade + message loop)
```

### 1.2 Message Protocol

**File: `catalog-server/src/server/websocket/messages.rs`**

A generic envelope format that can carry different feature-specific payloads:

```rust
use serde::{Deserialize, Serialize};

/// Wrapper for all server -> client messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerMessage {
    /// Message type identifier (e.g., "sync", "playback", "notification")
    #[serde(rename = "type")]
    pub msg_type: String,
    /// Feature-specific payload (JSON value)
    pub payload: serde_json::Value,
}

impl ServerMessage {
    pub fn new(msg_type: impl Into<String>, payload: impl Serialize) -> Self {
        Self {
            msg_type: msg_type.into(),
            payload: serde_json::to_value(payload).unwrap_or(serde_json::Value::Null),
        }
    }
}

/// Wrapper for all client -> server messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientMessage {
    /// Message type identifier
    #[serde(rename = "type")]
    pub msg_type: String,
    /// Feature-specific payload (JSON value)
    pub payload: serde_json::Value,
}

/// System-level messages (not feature-specific)
pub mod system {
    use super::*;

    /// Sent immediately after connection is established
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Connected {
        pub device_id: String,
    }

    /// Heartbeat response
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Pong;

    /// Heartbeat request (client -> server)
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Ping;

    /// Error message
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Error {
        pub code: String,
        pub message: String,
    }
}
```

**Reserved message types:**
- `connected` — sent by server on successful connection
- `ping` / `pong` — heartbeat
- `error` — server error response

Feature-specific types (defined elsewhere, not in this plan):
- `sync.*` — user data sync messages
- `playback.*` — remote playback control
- `notification.*` — push notifications

### 1.3 Connection Manager

**File: `catalog-server/src/server/websocket/connection.rs`**

Tracks active WebSocket connections, organized by user and device:

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use super::messages::ServerMessage;

/// Identifies a specific connection
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct ConnectionId {
    pub user_id: usize,
    pub device_id: String,
}

/// Information about an active connection
struct ConnectionEntry {
    sender: mpsc::Sender<ServerMessage>,
    device_type: String,
}

/// Manages all active WebSocket connections
pub struct ConnectionManager {
    /// user_id -> (device_id -> connection entry)
    connections: RwLock<HashMap<usize, HashMap<String, ConnectionEntry>>>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self {
            connections: RwLock::new(HashMap::new()),
        }
    }

    /// Register a new connection. Returns a receiver for outgoing messages.
    pub async fn register(
        &self,
        user_id: usize,
        device_id: String,
        device_type: String,
    ) -> mpsc::Receiver<ServerMessage> {
        let (tx, rx) = mpsc::channel(32);

        let mut conns = self.connections.write().await;
        let user_conns = conns.entry(user_id).or_default();

        // If device already connected, old connection will be dropped
        user_conns.insert(device_id, ConnectionEntry {
            sender: tx,
            device_type,
        });

        rx
    }

    /// Unregister a connection (called on disconnect)
    pub async fn unregister(&self, user_id: usize, device_id: &str) {
        let mut conns = self.connections.write().await;
        if let Some(user_conns) = conns.get_mut(&user_id) {
            user_conns.remove(device_id);
            if user_conns.is_empty() {
                conns.remove(&user_id);
            }
        }
    }

    /// Send message to a specific device
    pub async fn send_to_device(
        &self,
        user_id: usize,
        device_id: &str,
        message: ServerMessage,
    ) -> Result<(), SendError> {
        let conns = self.connections.read().await;
        if let Some(user_conns) = conns.get(&user_id) {
            if let Some(entry) = user_conns.get(device_id) {
                entry.sender.send(message).await
                    .map_err(|_| SendError::Disconnected)?;
                return Ok(());
            }
        }
        Err(SendError::NotConnected)
    }

    /// Send message to all OTHER devices of a user (excludes sender)
    /// Returns list of device_ids that failed (disconnected)
    pub async fn send_to_other_devices(
        &self,
        user_id: usize,
        exclude_device_id: &str,
        message: ServerMessage,
    ) -> Vec<String> {
        let conns = self.connections.read().await;
        let mut failed = Vec::new();

        if let Some(user_conns) = conns.get(&user_id) {
            for (device_id, entry) in user_conns.iter() {
                if device_id != exclude_device_id {
                    if entry.sender.send(message.clone()).await.is_err() {
                        failed.push(device_id.clone());
                    }
                }
            }
        }

        failed
    }

    /// Send message to ALL devices of a user
    /// Returns list of device_ids that failed
    pub async fn broadcast_to_user(
        &self,
        user_id: usize,
        message: ServerMessage,
    ) -> Vec<String> {
        let conns = self.connections.read().await;
        let mut failed = Vec::new();

        if let Some(user_conns) = conns.get(&user_id) {
            for (device_id, entry) in user_conns.iter() {
                if entry.sender.send(message.clone()).await.is_err() {
                    failed.push(device_id.clone());
                }
            }
        }

        failed
    }

    /// Get list of connected device IDs for a user
    pub async fn get_connected_devices(&self, user_id: usize) -> Vec<String> {
        let conns = self.connections.read().await;
        conns.get(&user_id)
            .map(|user_conns| user_conns.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Check if a specific device is connected
    pub async fn is_device_connected(&self, user_id: usize, device_id: &str) -> bool {
        let conns = self.connections.read().await;
        conns.get(&user_id)
            .map(|user_conns| user_conns.contains_key(device_id))
            .unwrap_or(false)
    }
}

#[derive(Debug)]
pub enum SendError {
    NotConnected,
    Disconnected,
}
```

### 1.4 WebSocket Handler

**File: `catalog-server/src/server/websocket/handler.rs`**

Handles the WebSocket upgrade and message loop:

```rust
use axum::{
    extract::{State, WebSocketUpgrade},
    extract::ws::{Message, WebSocket},
    response::Response,
};
use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use super::{
    connection::ConnectionManager,
    messages::{ServerMessage, ClientMessage, system},
};
use crate::server::session::Session;

/// State needed for WebSocket handling
pub struct WsState {
    pub connection_manager: Arc<ConnectionManager>,
    // Future: add message handlers/dispatchers here
}

/// WebSocket upgrade handler
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    session: Session,
    State(state): State<Arc<WsState>>,
) -> Response {
    // Configure WebSocket with idle timeout
    // Protocol-level ping/pong handles keepalive; tungstenite drops unresponsive connections
    ws.on_upgrade(move |socket| handle_socket(socket, session, state))
}

// Note: Axum/tungstenite automatically responds to WebSocket ping frames.
// For idle timeout, we can either:
// 1. Configure at reverse proxy level (nginx proxy_read_timeout)
// 2. Implement application-level timeout using tokio::time::timeout on recv()
// 3. Use axum's WebSocket config if available in the version we use

async fn handle_socket(
    socket: WebSocket,
    session: Session,
    state: Arc<WsState>,
) {
    let user_id = session.user_id;
    let device_id = session.device_id.clone();
    let device_type = session.device_type.clone();

    // Register connection and get receiver for outgoing messages
    let mut outgoing_rx = state.connection_manager
        .register(user_id, device_id.clone(), device_type)
        .await;

    let (mut ws_sink, mut ws_stream) = socket.split();

    // Send connected message
    let connected_msg = ServerMessage::new("connected", system::Connected {
        device_id: device_id.clone(),
    });
    if let Ok(json) = serde_json::to_string(&connected_msg) {
        let _ = ws_sink.send(Message::Text(json.into())).await;
    }

    // Spawn task to forward outgoing messages to WebSocket
    let outgoing_handle = tokio::spawn(async move {
        while let Some(msg) = outgoing_rx.recv().await {
            if let Ok(json) = serde_json::to_string(&msg) {
                if ws_sink.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
        }
    });

    // Process incoming messages
    while let Some(result) = ws_stream.next().await {
        match result {
            Ok(Message::Text(text)) => {
                if let Ok(msg) = serde_json::from_str::<ClientMessage>(&text) {
                    handle_client_message(&state, user_id, &device_id, msg).await;
                }
            }
            Ok(Message::Ping(data)) => {
                // Axum handles pong automatically, but we can log if needed
            }
            Ok(Message::Close(_)) => break,
            Err(_) => break,
            _ => {}
        }
    }

    // Cleanup
    outgoing_handle.abort();
    state.connection_manager.unregister(user_id, &device_id).await;
}

async fn handle_client_message(
    state: &WsState,
    user_id: usize,
    device_id: &str,
    msg: ClientMessage,
) {
    match msg.msg_type.as_str() {
        "ping" => {
            // Respond with pong
            let pong = ServerMessage::new("pong", system::Pong);
            let _ = state.connection_manager
                .send_to_device(user_id, device_id, pong)
                .await;
        }
        _ => {
            // Future: dispatch to feature-specific handlers
            // e.g., if msg_type starts with "sync." -> sync handler
            // e.g., if msg_type starts with "playback." -> playback handler
        }
    }
}
```

---

## Part 2: Server Integration

### 2.1 ServerState Changes

**File: `catalog-server/src/server/state.rs`**

Add the connection manager to server state:

```rust
use crate::server::websocket::connection::ConnectionManager;

pub struct ServerState {
    // ... existing fields ...
    pub ws_connection_manager: Arc<ConnectionManager>,
}
```

### 2.2 Route Registration

**File: `catalog-server/src/server/server.rs`**

Add the WebSocket route:

```rust
use crate::server::websocket::handler::{ws_handler, WsState};

// In router setup:
let ws_state = Arc::new(WsState {
    connection_manager: Arc::clone(&state.ws_connection_manager),
});

let app = Router::new()
    // ... existing routes ...
    .route("/v1/ws", get(ws_handler))
    .with_state(ws_state);
```

**Endpoint:** `GET /v1/ws`

Authentication: Uses the same session validation as REST endpoints (cookie or Authorization header).

### 2.3 Module Exports

**File: `catalog-server/src/server/websocket/mod.rs`**

```rust
pub mod messages;
pub mod connection;
pub mod handler;

pub use connection::ConnectionManager;
pub use messages::{ServerMessage, ClientMessage};
```

**File: `catalog-server/src/server/mod.rs`**

```rust
pub mod websocket;
```

---

## Part 3: Client Integration (Web)

### 3.1 WebSocket Service

**New file: `web/src/services/websocket.js`**

```javascript
import { ref, computed } from 'vue';
import { useAuthStore } from '@/store/auth';

// Connection state
const socket = ref(null);
const connected = ref(false);
const deviceId = ref(null);

// Message handlers by type prefix
const handlers = new Map();

/**
 * Register a handler for messages with a given type prefix
 * @param {string} typePrefix - e.g., "sync" handles "sync.liked", "sync.playlist", etc.
 * @param {function} handler - receives (fullType, payload)
 */
export function registerHandler(typePrefix, handler) {
  handlers.set(typePrefix, handler);
}

/**
 * Unregister a handler
 */
export function unregisterHandler(typePrefix) {
  handlers.delete(typePrefix);
}

/**
 * Connect to WebSocket server
 */
export function connect() {
  if (socket.value) {
    return; // Already connected or connecting
  }

  const authStore = useAuthStore();
  if (!authStore.isAuthenticated) {
    return;
  }

  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  const baseUrl = import.meta.env.VITE_API_BASE_URL || '';
  const wsUrl = baseUrl
    ? `${protocol}//${new URL(baseUrl).host}/v1/ws`
    : `${protocol}//${window.location.host}/v1/ws`;

  socket.value = new WebSocket(wsUrl);

  socket.value.onopen = () => {
    console.log('[WS] Connected');
  };

  socket.value.onmessage = (event) => {
    try {
      const msg = JSON.parse(event.data);
      handleMessage(msg);
    } catch (e) {
      console.error('[WS] Failed to parse message:', e);
    }
  };

  socket.value.onclose = (event) => {
    console.log('[WS] Disconnected:', event.code, event.reason);
    connected.value = false;
    deviceId.value = null;
    socket.value = null;

    // Auto-reconnect after delay (unless intentional close)
    if (event.code !== 1000) {
      setTimeout(() => {
        const authStore = useAuthStore();
        if (authStore.isAuthenticated) {
          connect();
        }
      }, 3000);
    }
  };

  socket.value.onerror = (error) => {
    console.error('[WS] Error:', error);
  };
}

/**
 * Disconnect from WebSocket server
 */
export function disconnect() {
  if (socket.value) {
    socket.value.close(1000, 'Client disconnect');
    socket.value = null;
    connected.value = false;
    deviceId.value = null;
  }
}

/**
 * Send a message to the server
 */
export function send(type, payload) {
  if (socket.value && socket.value.readyState === WebSocket.OPEN) {
    socket.value.send(JSON.stringify({ type, payload }));
  }
}

/**
 * Handle incoming message
 */
function handleMessage(msg) {
  const { type, payload } = msg;

  // Handle system messages
  if (type === 'connected') {
    connected.value = true;
    deviceId.value = payload.device_id;
    console.log('[WS] Registered as device:', payload.device_id);
    return;
  }

  if (type === 'pong') {
    return; // Heartbeat response
  }

  if (type === 'error') {
    console.error('[WS] Server error:', payload.code, payload.message);
    return;
  }

  // Dispatch to feature handlers by prefix
  const prefix = type.split('.')[0];
  const handler = handlers.get(prefix);
  if (handler) {
    handler(type, payload);
  } else {
    console.warn('[WS] No handler for message type:', type);
  }
}

// Export reactive state
export const wsConnected = computed(() => connected.value);
export const wsDeviceId = computed(() => deviceId.value);
```

### 3.2 Integration with Auth

**Modify: `web/src/store/auth.js`**

```javascript
import * as ws from '@/services/websocket';

// In login success:
async function login(username, password) {
  // ... existing login logic ...

  // Connect WebSocket after successful login
  ws.connect();
}

// In logout:
function logout() {
  ws.disconnect();
  // ... existing logout logic ...
}

// On app initialization (if already logged in):
function initialize() {
  if (isAuthenticated.value) {
    ws.connect();
  }
}
```

---

## Implementation Phases

### Phase 1: Server WebSocket Module
1. Create `websocket/` module structure
2. Implement `messages.rs` with envelope types
3. Implement `ConnectionManager` in `connection.rs`
4. Implement `ws_handler` in `handler.rs`
5. Export module in `server/mod.rs`

### Phase 2: Server Integration
1. Add `ws_connection_manager` to `ServerState`
2. Initialize `ConnectionManager` on server startup
3. Register `/v1/ws` route
4. Test connection with simple WebSocket client

### Phase 3: Web Client
1. Create `websocket.js` service
2. Integrate with auth store (connect on login, disconnect on logout)
3. Implement handler registration system
4. Test connection from browser

### Phase 4: Testing
1. Unit tests for `ConnectionManager`
2. Integration tests for WebSocket upgrade + authentication
3. Test reconnection behavior
4. Test multi-device scenarios

---

## Files Summary

### Must Create

| File | Description |
|------|-------------|
| `catalog-server/src/server/websocket/mod.rs` | Module exports |
| `catalog-server/src/server/websocket/messages.rs` | Message envelope types |
| `catalog-server/src/server/websocket/connection.rs` | Connection manager |
| `catalog-server/src/server/websocket/handler.rs` | WS route handler |
| `web/src/services/websocket.js` | Client WS service |

### Must Modify

| File | Changes |
|------|---------|
| `catalog-server/src/server/mod.rs` | Export websocket module |
| `catalog-server/src/server/state.rs` | Add ws_connection_manager |
| `catalog-server/src/server/server.rs` | Register /v1/ws route |
| `web/src/store/auth.js` | Connect/disconnect WS on auth changes |

---

## Dependencies

No new Cargo dependencies required:
- `axum` 0.8+ includes WebSocket support via `axum::extract::ws`
- `tokio` provides `mpsc` channels
- `futures` for stream handling (already a dependency)
- `serde_json` for message serialization (already a dependency)

---

## Extension Points

This infrastructure is designed to be extended by other features:

### Adding a New Feature (e.g., Sync)

1. **Define feature-specific message types** in a separate module:
   ```rust
   // In catalog-server/src/server/sync/messages.rs
   pub enum SyncMessage {
       ContentLiked { content_type: String, content_id: String },
       // ...
   }
   ```

2. **Register a message handler** in the WS handler:
   ```rust
   // In handle_client_message:
   if msg_type.starts_with("sync.") {
       sync_handler.handle(user_id, device_id, msg).await;
   }
   ```

3. **Use ConnectionManager to send messages**:
   ```rust
   // After a user action that needs to notify other devices:
   let msg = ServerMessage::new("sync.content_liked", payload);
   ws_connection_manager.send_to_other_devices(user_id, source_device_id, msg).await;
   ```

4. **Register handler on client**:
   ```javascript
   // In web/src/store/sync.js
   import { registerHandler } from '@/services/websocket';

   registerHandler('sync', (type, payload) => {
       // Handle sync messages
   });
   ```

---

## Pre-Implementation Verification

Before starting implementation, verify these assumptions:

- [ ] **Auth mechanisms work for WS upgrade**: Confirm that session extraction from cookies and Authorization header works during WebSocket upgrade (same middleware as REST)
- [ ] **Device limit exists**: Verify there's already a per-user device limit from the Device Entity feature. If not, determine the limit (suggested: 10 devices)

---

## Testing Checklist

- [ ] WebSocket upgrade succeeds with valid session cookie
- [ ] WebSocket upgrade succeeds with valid Authorization header
- [ ] WebSocket upgrade fails without authentication (401)
- [ ] `connected` message received on successful connection
- [ ] `ping` / `pong` heartbeat works (protocol-level)
- [ ] Idle connections are dropped after timeout
- [ ] Connection properly removed on disconnect
- [ ] Multiple devices for same user tracked separately
- [ ] Same device reconnecting replaces old connection (drop-and-replace)
- [ ] `send_to_other_devices` excludes source device
- [ ] `broadcast_to_user` reaches all devices
- [ ] Client auto-reconnects after unexpected disconnect
- [ ] Client doesn't reconnect after intentional disconnect (logout)
