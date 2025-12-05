# WebSocket Infrastructure - Implementation Tasks

This document tracks all implementation tasks for the WebSocket infrastructure feature.

**Legend:**
- `[ ]` Undone
- `[~]` In progress
- `[x]` Done

---

## Phase 0: Pre-Implementation Verification

Before writing any code, verify assumptions from the plan.

### 0.1 [x] Verify auth mechanisms work for WS upgrade

**Context:** WebSocket upgrade requests need to be authenticated using the same mechanisms as REST endpoints (cookies for browsers, Authorization header for native clients).

**Tasks:**
- [x] 0.1.1 Check how `Session` extractor works in axum - does it run on WS upgrade requests?
- [x] 0.1.2 Verify cookies are accessible during WS upgrade
- [x] 0.1.3 Verify Authorization header is accessible during WS upgrade
- [x] 0.1.4 Document any adjustments needed for WS-specific auth handling

**Files to check:**
- `catalog-server/src/server/session.rs`
- Axum documentation for WebSocket + extractors

**Findings:**
- `Session` uses `FromRequestParts<ServerState>` trait (session.rs:149) which extracts from HTTP request parts before body consumption
- WebSocket upgrade is an HTTP request, so `FromRequestParts` extractors work normally
- Cookies: extracted via `CookieJar::from_request_parts` (session.rs:48-58) - browsers send cookies with WS upgrade
- Authorization header: extracted directly from `parts.headers` (session.rs:60-66) - works for native clients
- **No adjustments needed** - existing Session extractor works as-is for WebSocket handlers

---

### 0.2 [x] Verify device limit exists

**Context:** There should already be a per-user device limit from the Device Entity feature. If not, we need to define one.

**Tasks:**
- [x] 0.2.1 Check if device limit is enforced in the device registration/login flow
- [x] 0.2.2 Document current limit (or decide on one if missing - suggested: 10 devices)
- [x] 0.2.3 Confirm how the limit affects WS connections (ConnectionManager doesn't need to enforce it if login already does)

**Files to check:**
- `catalog-server/src/user/` - device-related code
- Device Entity plan/implementation

**Findings:**
- Device limit exists: `MAX_DEVICES_PER_USER = 50` defined at `server.rs:55`
- Enforced during login via `enforce_user_device_limit()` at `server.rs:1075`
- When limit exceeded, oldest devices (by last_seen) are pruned
- **ConnectionManager doesn't need device limits** - login flow already enforces it, and WS connections are tied to authenticated devices

---

## Phase 1: Server WebSocket Module

Create the core WebSocket module with message types, connection manager, and handler.

### 1.1 [x] Create websocket module directory structure

**Context:** New module at `catalog-server/src/server/websocket/`

**Tasks:**
- [x] 1.1.1 Create directory `catalog-server/src/server/websocket/`
- [x] 1.1.2 Create empty `mod.rs` file

---

### 1.2 [x] Implement message types (`messages.rs`)

**Context:** Generic envelope format for all WS messages. Uses `type` field for routing and `payload` for feature-specific data.

**File:** `catalog-server/src/server/websocket/messages.rs`

**Tasks:**
- [x] 1.2.1 Create `ServerMessage` struct with `msg_type: String` and `payload: serde_json::Value`
- [x] 1.2.2 Implement `ServerMessage::new()` helper method
- [x] 1.2.3 Create `ClientMessage` struct (same structure as ServerMessage)
- [x] 1.2.4 Create `system` submodule with:
  - `Connected` struct (device_id)
  - `Ping` struct (unit)
  - `Pong` struct (unit)
  - `Error` struct (code, message)
- [x] 1.2.5 Add serde derives with `#[serde(rename = "type")]` for msg_type field
- [x] 1.2.6 Write unit tests for serialization/deserialization

**Reserved message types:**
- `connected` - server sends on successful connection
- `ping` / `pong` - application-level heartbeat
- `error` - server error response

---

### 1.3 [x] Implement ConnectionManager (`connection.rs`)

**Context:** Tracks all active WebSocket connections, organized by user_id -> device_id -> channel sender.

**File:** `catalog-server/src/server/websocket/connection.rs`

**Tasks:**
- [x] 1.3.1 Define `ConnectionId` struct (user_id, device_id) - Not needed, using separate params
- [x] 1.3.2 Define `ConnectionEntry` struct (sender: mpsc::Sender, device_type: String)
- [x] 1.3.3 Define `SendError` enum (NotConnected, Disconnected)
- [x] 1.3.4 Create `ConnectionManager` struct with `RwLock<HashMap<usize, HashMap<usize, ConnectionEntry>>>`
- [x] 1.3.5 Implement `ConnectionManager::new()`
- [x] 1.3.6 Implement `register()` - creates channel, stores sender, returns receiver
  - Must handle drop-and-replace for existing device connections
- [x] 1.3.7 Implement `unregister()` - removes connection entry, cleans up empty user maps
- [x] 1.3.8 Implement `send_to_device()` - send to specific device
- [x] 1.3.9 Implement `send_to_other_devices()` - broadcast excluding source device, returns failed device_ids
- [x] 1.3.10 Implement `broadcast_to_user()` - send to all devices of a user, returns failed device_ids
- [x] 1.3.11 Implement `get_connected_devices()` - list connected device_ids for a user
- [x] 1.3.12 Implement `is_device_connected()` - check if specific device is connected
- [x] 1.3.13 Write unit tests for all ConnectionManager methods (13 tests)

**Design notes:**
- Channel buffer size: 32 messages
- Drop-and-replace: if device_id already exists, old sender is replaced (old connection will error on next send)

---

### 1.4 [x] Implement WebSocket handler (`handler.rs`)

**Context:** Handles WS upgrade, message loop, and cleanup.

**File:** `catalog-server/src/server/websocket/handler.rs`

**Tasks:**
- [x] 1.4.1 Define `WsState` struct containing `Arc<ConnectionManager>`
- [x] 1.4.2 Implement `ws_handler()` - the axum route handler
  - Extract `WebSocketUpgrade`, `Session`, and `State<Arc<WsState>>`
  - Call `ws.on_upgrade()` with `handle_socket`
  - Reject if device_id is missing (400 Bad Request)
- [x] 1.4.3 Implement `handle_socket()` async function:
  - [x] 1.4.3a Extract user_id, device_id, device_type from session
  - [x] 1.4.3b Register with ConnectionManager, get outgoing_rx
  - [x] 1.4.3c Split socket into sink and stream
  - [x] 1.4.3d Send `connected` message immediately
  - [x] 1.4.3e Spawn task to forward outgoing_rx messages to ws_sink
  - [x] 1.4.3f Loop on ws_stream to receive incoming messages
  - [x] 1.4.3g Handle Close and Error by breaking loop
  - [x] 1.4.3h On exit: abort outgoing task, unregister from ConnectionManager
- [x] 1.4.4 Implement `handle_client_message()`:
  - Handle `ping` -> respond with `pong`
  - Handle unknown types -> send error message
  - Placeholder for future feature dispatch (sync.*, playback.*)
- [ ] 1.4.5 Add idle timeout handling - Deferred (can rely on reverse proxy or add later)
- [ ] 1.4.6 Write integration test for basic connect/disconnect flow - Deferred to Phase 4

---

### 1.5 [x] Create module exports (`mod.rs`)

**File:** `catalog-server/src/server/websocket/mod.rs`

**Tasks:**
- [x] 1.5.1 Add `pub mod messages;`
- [x] 1.5.2 Add `pub mod connection;`
- [x] 1.5.3 Add `pub mod handler;`
- [x] 1.5.4 Add re-exports: `pub use connection::ConnectionManager;`
- [x] 1.5.5 Add re-exports: `pub use messages::{ServerMessage, ClientMessage};`
- [x] 1.5.6 Add re-exports: `pub use handler::{ws_handler, WsState};`

**Note:** This was done incrementally while implementing each module.

---

## Phase 2: Server Integration

Integrate the WebSocket module into the main server.

### 2.1 [x] Export websocket module from server

**File:** `catalog-server/src/server/mod.rs`

**Tasks:**
- [x] 2.1.1 Add `pub mod websocket;`

**Note:** Already done in Task 1.2.

---

### 2.2 [x] Add ConnectionManager to ServerState

**File:** `catalog-server/src/server/state.rs` (or wherever ServerState is defined)

**Tasks:**
- [x] 2.2.1 Add `ws_connection_manager: Arc<ConnectionManager>` field to ServerState
- [x] 2.2.2 Initialize ConnectionManager in ServerState constructor/builder (server.rs:1876)
- [x] 2.2.3 Add `GuardedConnectionManager` type alias
- [x] 2.2.4 Add `FromRef` impl for `GuardedConnectionManager`

---

### 2.3 [x] Register WebSocket route

**File:** `catalog-server/src/server/server.rs`

**Tasks:**
- [x] 2.3.1 Import websocket handler (using super::websocket::ws_handler)
- [x] 2.3.2 Handler extracts GuardedConnectionManager via FromRef (simplified from WsState)
- [x] 2.3.3 Add route: `.nest("/v1", ws_routes)` with `.route("/ws", get(ws_handler))`
- [x] 2.3.4 Session extractor validates auth automatically

**Endpoint:** `GET /v1/ws`

---

### 2.4 [x] Verify server compiles and starts

**Tasks:**
- [x] 2.4.1 Run `cargo build` - Success
- [x] 2.4.2 Run `cargo test` - All tests pass
- [ ] 2.4.3 Start server and verify no startup errors - Manual verification needed

---

## Phase 3: Web Client Integration

Implement the WebSocket service for the Vue frontend.

### 3.1 [x] Create WebSocket service

**File:** `web/src/services/websocket.js`

**Tasks:**
- [x] 3.1.1 Create file with module-level state:
  - `socket` ref (WebSocket instance or null)
  - `connected` ref (boolean)
  - `deviceId` ref (string or null)
  - `handlers` Map (typePrefix -> handler function)
- [x] 3.1.2 Implement `registerHandler(typePrefix, handler)`
- [x] 3.1.3 Implement `unregisterHandler(typePrefix)`
- [x] 3.1.4 Implement `connect()`:
  - Guard: return if already connected or not authenticated
  - Build WS URL from current location (ws/wss based on http/https)
  - Create WebSocket instance
  - Set up onopen, onmessage, onclose, onerror handlers
  - Auto-reconnect on unexpected close (3 second delay)
- [x] 3.1.5 Implement `disconnect()`:
  - Close with code 1000 (normal closure)
  - Clear socket and state
- [x] 3.1.6 Implement `send(type, payload)`:
  - Guard: only send if socket is open
  - JSON stringify and send
- [x] 3.1.7 Implement `handleMessage(msg)`:
  - Handle `connected` -> set connected=true, store deviceId
  - Handle `pong` -> ignore (heartbeat response)
  - Handle `error` -> console.error
  - Dispatch to feature handlers by type prefix
- [x] 3.1.8 Export reactive computed: `wsConnected`, `wsDeviceId`

---

### 3.2 [ ] Integrate with auth store

**File:** `web/src/store/auth.js`

**Tasks:**
- [ ] 3.2.1 Import websocket service
- [ ] 3.2.2 Call `ws.connect()` after successful login
- [ ] 3.2.3 Call `ws.disconnect()` on logout
- [ ] 3.2.4 Call `ws.connect()` in initialize() if already authenticated

---

### 3.3 [ ] Test web client connection

**Tasks:**
- [ ] 3.3.1 Start server and web dev server
- [ ] 3.3.2 Log in and verify WS connection in browser dev tools
- [ ] 3.3.3 Verify `connected` message received
- [ ] 3.3.4 Log out and verify WS disconnects cleanly
- [ ] 3.3.5 Test auto-reconnect by killing server and restarting

---

## Phase 4: Testing

Comprehensive testing of the WebSocket infrastructure.

### 4.1 [ ] Unit tests for ConnectionManager

**File:** `catalog-server/src/server/websocket/connection.rs` (tests module)

**Tasks:**
- [ ] 4.1.1 Test `register()` creates valid receiver
- [ ] 4.1.2 Test `unregister()` removes connection
- [ ] 4.1.3 Test `send_to_device()` delivers message
- [ ] 4.1.4 Test `send_to_device()` returns NotConnected for unknown device
- [ ] 4.1.5 Test `send_to_other_devices()` excludes source device
- [ ] 4.1.6 Test `send_to_other_devices()` returns failed devices
- [ ] 4.1.7 Test `broadcast_to_user()` sends to all devices
- [ ] 4.1.8 Test `get_connected_devices()` returns correct list
- [ ] 4.1.9 Test `is_device_connected()` returns correct boolean
- [ ] 4.1.10 Test drop-and-replace: registering same device_id replaces old connection

---

### 4.2 [ ] Unit tests for message serialization

**File:** `catalog-server/src/server/websocket/messages.rs` (tests module)

**Tasks:**
- [ ] 4.2.1 Test ServerMessage serializes to expected JSON format
- [ ] 4.2.2 Test ClientMessage deserializes from JSON
- [ ] 4.2.3 Test system::Connected serializes correctly
- [ ] 4.2.4 Test system::Error serializes correctly

---

### 4.3 [ ] Integration tests for WebSocket endpoint

**File:** `catalog-server/tests/websocket_test.rs` (or similar)

**Tasks:**
- [ ] 4.3.1 Test WS upgrade succeeds with valid session cookie
- [ ] 4.3.2 Test WS upgrade succeeds with valid Authorization header
- [ ] 4.3.3 Test WS upgrade fails without authentication (expect 401)
- [ ] 4.3.4 Test `connected` message received after successful upgrade
- [ ] 4.3.5 Test application-level ping/pong works
- [ ] 4.3.6 Test connection properly removed on client disconnect
- [ ] 4.3.7 Test same device reconnecting replaces old connection

---

### 4.4 [ ] Multi-device scenario tests

**Tasks:**
- [ ] 4.4.1 Test two devices for same user can connect simultaneously
- [ ] 4.4.2 Test `send_to_other_devices` reaches second device but not first
- [ ] 4.4.3 Test `broadcast_to_user` reaches both devices
- [ ] 4.4.4 Test one device disconnecting doesn't affect other

---

### 4.5 [ ] Web client manual testing

**Tasks:**
- [ ] 4.5.1 Test connection on login
- [ ] 4.5.2 Test disconnection on logout
- [ ] 4.5.3 Test auto-reconnect after network interruption
- [ ] 4.5.4 Test no reconnect after intentional logout
- [ ] 4.5.5 Test multiple browser tabs (each should connect independently)

---

## Phase 5: Documentation & Cleanup

Final polish before marking feature complete.

### 5.1 [ ] Update CLAUDE.md if needed

**Tasks:**
- [ ] 5.1.1 Add any new commands or patterns for WebSocket development
- [ ] 5.1.2 Document how to test WS connections manually

---

### 5.2 [ ] Code review checklist

**Tasks:**
- [ ] 5.2.1 All new code has appropriate error handling
- [ ] 5.2.2 No unwrap() on user-controlled data
- [ ] 5.2.3 Logging added for connection/disconnection events
- [ ] 5.2.4 No sensitive data logged (tokens, etc.)
- [ ] 5.2.5 All public APIs documented with doc comments

---

### 5.3 [ ] Clean up plan documents

**Tasks:**
- [ ] 5.3.1 Mark WS_CONNECTION_PLAN.md as implemented
- [ ] 5.3.2 Archive or delete this implementation tracking doc
- [ ] 5.3.3 Update TODO.md if WebSocket was listed there

---

## Summary

| Phase | Tasks | Status |
|-------|-------|--------|
| Phase 0: Verification | 2 groups | [ ] Not started |
| Phase 1: Server Module | 5 groups | [ ] Not started |
| Phase 2: Server Integration | 4 groups | [ ] Not started |
| Phase 3: Web Client | 3 groups | [ ] Not started |
| Phase 4: Testing | 5 groups | [ ] Not started |
| Phase 5: Cleanup | 3 groups | [ ] Not started |

**Total task groups:** 22
**Total individual tasks:** ~70
