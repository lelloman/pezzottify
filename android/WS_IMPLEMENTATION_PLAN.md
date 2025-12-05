# Android WebSocket Implementation Plan

This document outlines the implementation plan for adding WebSocket support to the Android app.

## Prerequisites

- Server WebSocket endpoint already implemented (`GET /v1/ws`)
- Device tracking in place (login sends device info, session includes device_id)
- Same message protocol as web client (`type` + `payload` JSON envelope)

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                         UI Layer                            │
│  ┌─────────────────┐  ┌─────────────────┐                   │
│  │ ConnectionStatus│  │   ViewModel     │                   │
│  │   Indicator     │  │ (observes state)│                   │
│  └────────┬────────┘  └────────┬────────┘                   │
│           │                    │                            │
│           └────────┬───────────┘                            │
│                    ▼                                        │
├─────────────────────────────────────────────────────────────┤
│                       Domain Layer                          │
│  ┌─────────────────────────────────────────────────────┐    │
│  │              WebSocketManager (interface)            │    │
│  │  - connectionState: StateFlow<ConnectionState>       │    │
│  │  - connect() / disconnect()                          │    │
│  │  - send(type, payload)                               │    │
│  │  - registerHandler(prefix, handler)                  │    │
│  └─────────────────────────────────────────────────────┘    │
├─────────────────────────────────────────────────────────────┤
│                      RemoteAPI Layer                        │
│  ┌─────────────────────────────────────────────────────┐    │
│  │           WebSocketManagerImpl (OkHttp)              │    │
│  │  - Uses OkHttp WebSocket API                         │    │
│  │  - Handles reconnection with exponential backoff     │    │
│  │  - Injects auth token from AuthStore                 │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

---

## Key Design Decisions

1. **Use OkHttp WebSocket** - Already a dependency, no new libraries needed
2. **Interface in domain, implementation in remoteapi** - Clean architecture
3. **StateFlow for connection state** - Consistent with existing patterns
4. **Handler registration system** - Same pattern as web client for extensibility
5. **Auto-reconnect with exponential backoff** - Follow BaseSynchronizer pattern
6. **React to AuthState changes** - Connect on login, disconnect on logout

---

## Connection States

```kotlin
sealed interface ConnectionState {
    data object Disconnected : ConnectionState
    data object Connecting : ConnectionState
    data class Connected(val deviceId: Int) : ConnectionState
    data class Error(val message: String) : ConnectionState
}
```

Maps to UI indicator:
- `Disconnected` / `Error` → Red dot
- `Connecting` → Orange dot (pulsing)
- `Connected` → Green dot

---

## Phase 1: Domain Layer Interface

### 1.1 [ ] Create WebSocketManager interface

**File:** `android/domain/src/main/java/com/lelloman/pezzottify/android/domain/websocket/WebSocketManager.kt`

**Tasks:**
- [ ] 1.1.1 Create `ConnectionState` sealed interface
- [ ] 1.1.2 Create `WebSocketManager` interface with:
  - `connectionState: StateFlow<ConnectionState>`
  - `suspend fun connect()`
  - `suspend fun disconnect()`
  - `fun send(type: String, payload: Any?)`
  - `fun registerHandler(prefix: String, handler: MessageHandler)`
  - `fun unregisterHandler(prefix: String)`
- [ ] 1.1.3 Create `MessageHandler` functional interface

---

### 1.2 [ ] Create message types

**File:** `android/domain/src/main/java/com/lelloman/pezzottify/android/domain/websocket/WebSocketMessage.kt`

**Tasks:**
- [ ] 1.2.1 Create `ClientMessage` data class (type, payload)
- [ ] 1.2.2 Create `ServerMessage` data class (type, payload)
- [ ] 1.2.3 Add kotlinx.serialization annotations

---

## Phase 2: RemoteAPI Layer Implementation

### 2.1 [ ] Create WebSocketManagerImpl

**File:** `android/remoteapi/src/main/java/com/lelloman/pezzottify/android/remoteapi/internal/websocket/WebSocketManagerImpl.kt`

**Tasks:**
- [ ] 2.1.1 Create class implementing `WebSocketManager`
- [ ] 2.1.2 Inject dependencies:
  - `AuthStore` - for auth token
  - `HostUrlProvider` - for base URL (convert http→ws, https→wss)
  - `CoroutineScope` - for background work
  - `LoggerFactory` - for logging
- [ ] 2.1.3 Create `MutableStateFlow<ConnectionState>` for state
- [ ] 2.1.4 Create `handlers: ConcurrentHashMap<String, MessageHandler>` for message routing

---

### 2.2 [ ] Implement connection logic

**File:** `android/remoteapi/src/main/java/com/lelloman/pezzottify/android/remoteapi/internal/websocket/WebSocketManagerImpl.kt`

**Tasks:**
- [ ] 2.2.1 Implement `connect()`:
  - Check if already connected/connecting
  - Get auth token from AuthStore
  - Build WebSocket URL from base URL
  - Create OkHttp WebSocket with auth header
  - Set state to `Connecting`
- [ ] 2.2.2 Implement `disconnect()`:
  - Close WebSocket with code 1000
  - Set state to `Disconnected`
  - Cancel any pending reconnect
- [ ] 2.2.3 Implement `WebSocketListener`:
  - `onOpen` → wait for server "connected" message
  - `onMessage` → parse JSON, dispatch to handlers
  - `onClosed` → set state, schedule reconnect if unexpected
  - `onFailure` → set error state, schedule reconnect
- [ ] 2.2.4 Implement `send()`:
  - Serialize message to JSON
  - Send via WebSocket if connected

---

### 2.3 [ ] Implement reconnection logic

**Tasks:**
- [ ] 2.3.1 Add reconnection parameters:
  - `minBackoff = 1000ms`
  - `maxBackoff = 30000ms`
  - `backoffMultiplier = 1.5`
- [ ] 2.3.2 Track `reconnectAttempt` counter
- [ ] 2.3.3 Implement exponential backoff calculation
- [ ] 2.3.4 Use `CoroutineScope.launch` with `delay()` for retry
- [ ] 2.3.5 Reset backoff on successful connection
- [ ] 2.3.6 Add `intentionalDisconnect` flag to prevent reconnect after logout

---

### 2.4 [ ] Implement message handling

**Tasks:**
- [ ] 2.4.1 Parse incoming JSON to `ServerMessage`
- [ ] 2.4.2 Handle system messages:
  - `connected` → extract device_id, set state to `Connected`
  - `pong` → ignore (heartbeat response)
  - `error` → log error
- [ ] 2.4.3 Dispatch to handlers by prefix:
  - Extract prefix from type (e.g., "sync" from "sync.liked")
  - Call registered handler if exists
- [ ] 2.4.4 Implement `registerHandler()` and `unregisterHandler()`

---

### 2.5 [ ] Implement ping/pong heartbeat

**Tasks:**
- [ ] 2.5.1 Add heartbeat interval constant (30 seconds)
- [ ] 2.5.2 Launch coroutine to send ping periodically when connected
- [ ] 2.5.3 Cancel heartbeat coroutine on disconnect
- [ ] 2.5.4 Consider connection dead if no pong received (optional)

---

## Phase 3: Dependency Injection

### 3.1 [ ] Add WebSocketManager to DI

**File:** `android/remoteapi/src/main/java/com/lelloman/pezzottify/android/remoteapi/RemoteApiModule.kt`

**Tasks:**
- [ ] 3.1.1 Add `@Provides @Singleton` function for `WebSocketManager`
- [ ] 3.1.2 Inject required dependencies (AuthStore, HostUrlProvider, etc.)
- [ ] 3.1.3 Create application-scoped CoroutineScope for WebSocket

---

### 3.2 [ ] Export WebSocketManager from remoteapi module

**File:** `android/remoteapi/build.gradle.kts` (if needed)

**Tasks:**
- [ ] 3.2.1 Ensure domain module has access to WebSocketManager interface
- [ ] 3.2.2 Verify no circular dependencies

---

## Phase 4: Auth Integration

### 4.1 [ ] Connect WebSocket on login

**File:** `android/domain/src/main/java/com/lelloman/pezzottify/android/domain/auth/usecase/PerformLogin.kt`

**Tasks:**
- [ ] 4.1.1 Inject `WebSocketManager` into `PerformLogin`
- [ ] 4.1.2 Call `webSocketManager.connect()` after successful login
- [ ] 4.1.3 Ensure connection happens after auth state is stored

---

### 4.2 [ ] Disconnect WebSocket on logout

**File:** `android/domain/src/main/java/com/lelloman/pezzottify/android/domain/auth/usecase/PerformLogout.kt`

**Tasks:**
- [ ] 4.2.1 Inject `WebSocketManager` into `PerformLogout`
- [ ] 4.2.2 Call `webSocketManager.disconnect()` before clearing auth
- [ ] 4.2.3 Ensure intentional disconnect flag is set

---

### 4.3 [ ] Auto-connect on app start if authenticated

**File:** `android/domain/src/main/java/com/lelloman/pezzottify/android/domain/AppInitializer.kt` (or similar)

**Tasks:**
- [ ] 4.3.1 Create `WebSocketInitializer` implementing `AppInitializer`
- [ ] 4.3.2 Observe `AuthStore.getAuthState()`
- [ ] 4.3.3 Connect when state becomes `LoggedIn`
- [ ] 4.3.4 Disconnect when state becomes `LoggedOut`

---

## Phase 5: UI Status Indicator

### 5.1 [ ] Create ConnectionStatusIndicator composable

**File:** `android/ui/src/main/java/com/lelloman/pezzottify/android/ui/common/ConnectionStatusIndicator.kt`

**Tasks:**
- [ ] 5.1.1 Create `@Composable` function
- [ ] 5.1.2 Accept `connectionState: ConnectionState` parameter
- [ ] 5.1.3 Render colored dot:
  - Green (`#22c55e`) for `Connected`
  - Orange (`#f97316`) for `Connecting` (with pulse animation)
  - Red (`#ef4444`) for `Disconnected` / `Error`
- [ ] 5.1.4 Add subtle glow effect (optional)
- [ ] 5.1.5 Show tooltip on long press with status details

---

### 5.2 [ ] Add indicator to app bar/header

**File:** TBD (main scaffold or top bar)

**Tasks:**
- [ ] 5.2.1 Find the main app bar composable
- [ ] 5.2.2 Inject `WebSocketManager` into relevant ViewModel
- [ ] 5.2.3 Expose `connectionState` as StateFlow
- [ ] 5.2.4 Place `ConnectionStatusIndicator` in app bar

---

## Phase 6: Testing

### 6.1 [ ] Unit tests for WebSocketManagerImpl

**File:** `android/remoteapi/src/test/java/.../websocket/WebSocketManagerImplTest.kt`

**Tasks:**
- [ ] 6.1.1 Test connection state transitions
- [ ] 6.1.2 Test message parsing and handler dispatch
- [ ] 6.1.3 Test reconnection backoff logic
- [ ] 6.1.4 Test intentional disconnect prevents reconnect
- [ ] 6.1.5 Mock OkHttp WebSocket for unit tests

---

### 6.2 [ ] Integration tests

**File:** `android/remoteapi/src/integrationTest/java/.../websocket/WebSocketIntegrationTest.kt`

**Tasks:**
- [ ] 6.2.1 Test real connection to test server
- [ ] 6.2.2 Test "connected" message received
- [ ] 6.2.3 Test ping/pong heartbeat
- [ ] 6.2.4 Test reconnection after server restart

---

## Files Summary

### Must Create

| File | Description |
|------|-------------|
| `domain/.../websocket/WebSocketManager.kt` | Interface + ConnectionState |
| `domain/.../websocket/WebSocketMessage.kt` | Message data classes |
| `remoteapi/.../websocket/WebSocketManagerImpl.kt` | OkHttp implementation |
| `ui/.../common/ConnectionStatusIndicator.kt` | Status dot composable |

### Must Modify

| File | Changes |
|------|---------|
| `remoteapi/RemoteApiModule.kt` | Add WebSocketManager provider |
| `domain/.../auth/usecase/PerformLogin.kt` | Connect on login |
| `domain/.../auth/usecase/PerformLogout.kt` | Disconnect on logout |
| App bar/header composable | Add status indicator |

---

## Dependencies

**No new dependencies required:**
- OkHttp WebSocket API is included in existing OkHttp dependency
- kotlinx.serialization already used for JSON

**Existing dependencies to use:**
```kotlin
implementation(platform(libs.okhttp.bom))
implementation(libs.okhttp)
implementation(libs.kotlinx.serialization.json)
implementation(libs.kotlinx.coroutines.core)
```

---

## Message Protocol

Same as web client - JSON envelope format:

```json
// Client → Server
{"type": "ping", "payload": null}

// Server → Client
{"type": "connected", "payload": {"device_id": 123}}
{"type": "pong", "payload": null}
{"type": "error", "payload": {"code": "...", "message": "..."}}
```

---

## Implementation Order

1. **Phase 1** - Domain interfaces (foundation)
2. **Phase 2** - RemoteAPI implementation (core logic)
3. **Phase 3** - DI wiring (make it injectable)
4. **Phase 4** - Auth integration (connect/disconnect lifecycle)
5. **Phase 5** - UI indicator (user visibility)
6. **Phase 6** - Testing (quality assurance)

---

## Open Questions

1. **App backgrounding** - Should WebSocket disconnect when app goes to background? Or stay connected for notifications?
2. **Network changes** - Should we listen for connectivity changes and reconnect?
3. **Battery optimization** - Any concerns with persistent connection on mobile?

---

## Summary

| Phase | Tasks | Estimated Complexity |
|-------|-------|---------------------|
| Phase 1: Domain Interface | 3 groups | Low |
| Phase 2: Implementation | 5 groups | High |
| Phase 3: DI | 2 groups | Low |
| Phase 4: Auth Integration | 3 groups | Medium |
| Phase 5: UI Indicator | 2 groups | Medium |
| Phase 6: Testing | 2 groups | Medium |

**Total task groups:** 17
**Total individual tasks:** ~45
