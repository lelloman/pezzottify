/**
 * WebSocket service for real-time communication.
 *
 * Provides a generic WebSocket connection that can be extended
 * for features like user data sync, remote playback control, etc.
 */

import { ref, shallowRef, computed } from "vue";
import { onServerReady, powerStatus } from "./power.js";

// Connection state (module-level, singleton)
const socket = shallowRef(null);
const connected = ref(false);
const connecting = ref(false);
const deviceId = ref(null);
const serverVersion = ref(null);

// Message handlers by type prefix
const handlers = new Map();

// Retry and liveness work runs only while this tab is visible.
const RETRY_MS = 30000;
const CONNECTION_TIMEOUT_MS = 10000;
const PONG_TIMEOUT_MS = 10000;
let reconnectTimeout = null;
let connectionDesired = false;
let reconnectCheck = null;
let checkTimeout = null;
let connectionTimeout = null;
let heartbeatTimeout = null;
let pongTimeout = null;
let connectionStartedAt = 0;

function foreground() {
  return typeof document === "undefined" || !document.hidden;
}

function clearSocketTimers() {
  clearTimeout(connectionTimeout);
  clearTimeout(heartbeatTimeout);
  clearTimeout(pongTimeout);
  connectionTimeout = heartbeatTimeout = pongTimeout = null;
}

function cancelAvailabilityCheck() {
  const previous = reconnectCheck;
  reconnectCheck = null;
  clearTimeout(checkTimeout);
  checkTimeout = null;
  previous?.abort();
}

function resetSocket(ws, delay = RETRY_MS) {
  if (socket.value !== ws) return;
  clearSocketTimers();
  socket.value = null;
  connected.value = false;
  connecting.value = false;
  deviceId.value = null;
  serverVersion.value = null;
  try { ws.close(1000, "Connection recovery"); } catch { /* Already closed. */ }
  scheduleReconnect(delay);
}

function scheduleReconnect(delay = RETRY_MS) {
  if (!foreground() || !connectionDesired || socket.value || reconnectTimeout || reconnectCheck) return;
  reconnectTimeout = setTimeout(() => {
    reconnectTimeout = null;
    reconnectWhenReady();
  }, delay);
}

function reconnectWhenReady() {
  if (!foreground() || !connectionDesired || socket.value || reconnectCheck) return;
  clearTimeout(reconnectTimeout);
  reconnectTimeout = null;
  const check = new AbortController();
  reconnectCheck = check;
  checkTimeout = setTimeout(() => {
    if (reconnectCheck !== check) return;
    cancelAvailabilityCheck();
    scheduleReconnect();
  }, 5000);
  // Availability polling must never wake an otherwise idle server.
  powerStatus({ signal: check.signal })
    .then((state) => {
      if (reconnectCheck === check && foreground() && state === "ready" && connectionDesired && !socket.value) connect();
    })
    .catch(() => {})
    .finally(() => {
      if (reconnectCheck !== check) return;
      reconnectCheck = null;
      clearTimeout(checkTimeout);
      checkTimeout = null;
      scheduleReconnect();
    });
}

function scheduleHeartbeat() {
  clearTimeout(heartbeatTimeout);
  heartbeatTimeout = null;
  if (foreground() && connected.value) {
    heartbeatTimeout = setTimeout(probeSocket, RETRY_MS);
  }
}

function probeSocket() {
  heartbeatTimeout = null;
  const ws = socket.value;
  if (!foreground() || !connectionDesired || !ws || pongTimeout) return;
  if (!connected.value || ws.readyState !== WebSocket.OPEN) {
    resetSocket(ws, 0);
    return;
  }
  pongTimeout = setTimeout(() => resetSocket(ws, 0), PONG_TIMEOUT_MS);
  try { ws.send(JSON.stringify({ type: "ping", payload: null })); }
  catch { resetSocket(ws, 0); }
}

function armConnectionTimeout(ws) {
  clearTimeout(connectionTimeout);
  connectionTimeout = null;
  if (!foreground()) return;
  const remaining = CONNECTION_TIMEOUT_MS - (Date.now() - connectionStartedAt);
  if (remaining <= 0) resetSocket(ws, 0);
  else connectionTimeout = setTimeout(() => resetSocket(ws), remaining);
}

function resumeConnection() {
  if (!foreground()) {
    clearTimeout(reconnectTimeout);
    reconnectTimeout = null;
    cancelAvailabilityCheck();
    clearSocketTimers();
    return;
  }
  if (!connectionDesired) return;
  const ws = socket.value;
  if (!ws) reconnectWhenReady();
  else if (connected.value && ws.readyState === WebSocket.OPEN) {
    clearTimeout(heartbeatTimeout);
    heartbeatTimeout = null;
    probeSocket();
  } else if (ws.readyState === WebSocket.CONNECTING || ws.readyState === WebSocket.OPEN) {
    armConnectionTimeout(ws);
  } else resetSocket(ws, 0);
}

onServerReady(resumeConnection);

if (typeof window !== "undefined") {
  window.addEventListener("online", resumeConnection);
  window.addEventListener("focus", resumeConnection);
  window.addEventListener("pageshow", resumeConnection);
}
if (typeof document !== "undefined") {
  document.addEventListener("visibilitychange", resumeConnection);
  document.addEventListener("freeze", () => {
    clearTimeout(reconnectTimeout);
    reconnectTimeout = null;
    cancelAvailabilityCheck();
    clearSocketTimers();
  });
  document.addEventListener("resume", resumeConnection);
}

/**
 * Register a handler for messages with a given type prefix.
 * @param {string} typePrefix - e.g., "sync" handles "sync.liked", "sync.playlist", etc.
 * @param {function} handler - receives (fullType, payload)
 */
export function registerHandler(typePrefix, handler) {
  handlers.set(typePrefix, handler);
}

/**
 * Unregister a handler.
 * @param {string} typePrefix
 */
export function unregisterHandler(typePrefix) {
  handlers.delete(typePrefix);
}

/**
 * Build the WebSocket URL based on current location.
 * @returns {string}
 */
function buildWsUrl() {
  const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  return `${protocol}//${window.location.host}/v1/ws?wake=passive`;
}

/**
 * Connect to the WebSocket server.
 * Requires authentication - call after successful login.
 */
export function connect() {
  connectionDesired = true;
  if (!foreground()) return;
  if (socket.value) {
    resumeConnection();
    return;
  }

  // Clear any pending reconnect
  if (reconnectTimeout) {
    clearTimeout(reconnectTimeout);
    reconnectTimeout = null;
  }

  connecting.value = true;

  const wsUrl = buildWsUrl();
  console.log("[WS] Connecting to", wsUrl);

  const ws = new WebSocket(wsUrl);
  socket.value = ws;
  connectionStartedAt = Date.now();
  armConnectionTimeout(ws);

  ws.onopen = () => {
    if (socket.value !== ws) return;
    console.log("[WS] Connection opened, waiting for server confirmation...");
  };

  ws.onmessage = (event) => {
    if (socket.value !== ws) return;
    try {
      const msg = JSON.parse(event.data);
      if (msg.type === "connected") {
        clearTimeout(connectionTimeout);
        connectionTimeout = null;
      }
      if (msg.type === "pong") {
        clearTimeout(pongTimeout);
        pongTimeout = null;
        scheduleHeartbeat();
      }
      handleMessage(msg);
      if (msg.type === "connected") scheduleHeartbeat();
    } catch (e) {
      console.error("[WS] Failed to parse message:", e);
    }
  };

  ws.onclose = (event) => {
    if (socket.value !== ws) return;
    console.log("[WS] Connection closed:", event.code, event.reason);
    resetSocket(ws);
  };

  ws.onerror = (error) => {
    if (socket.value !== ws) return;
    console.error("[WS] Error:", error);
  };
}

/**
 * Disconnect from the WebSocket server.
 * Call on logout.
 */
export function disconnect() {
  connectionDesired = false;
  clearTimeout(reconnectTimeout);
  reconnectTimeout = null;
  cancelAvailabilityCheck();
  clearSocketTimers();
  if (socket.value) resetSocket(socket.value);

}

/**
 * Send a message to the server.
 * @param {string} type - Message type
 * @param {*} payload - Message payload (will be JSON serialized)
 */
export function send(type, payload = null) {
  if (socket.value && socket.value.readyState === WebSocket.OPEN) {
    const msg = { type, payload };
    socket.value.send(JSON.stringify(msg));
  } else {
    console.warn("[WS] Cannot send, not connected");
  }
}

/**
 * Send a ping to the server.
 */
export function ping() {
  send("ping", null);
}

/**
 * Handle incoming message.
 * @param {Object} msg - Parsed message with type and payload
 */
function handleMessage(msg) {
  const { type, payload } = msg;
  console.log("[WS] Received message:", type, payload);

  // Handle system messages
  if (type === "connected") {
    connected.value = true;
    connecting.value = false;
    deviceId.value = payload.device_id;
    serverVersion.value = payload.server_version;
    console.log(
      "[WS] Connected as device:",
      payload.device_id,
      "server version:",
      payload.server_version,
    );
    return;
  }

  if (type === "pong") {
    // Heartbeat response - ignore
    return;
  }

  if (type === "error") {
    console.error("[WS] Server error:", payload.code, payload.message);
    return;
  }

  // Dispatch to feature handlers by prefix
  const prefix = type.split(".")[0];
  const handler = handlers.get(prefix);
  if (handler) {
    try {
      handler(type, payload);
    } catch (e) {
      console.error("[WS] Handler error for", type, ":", e);
    }
  } else {
    console.warn("[WS] No handler for message type:", type);
  }
}

// Export reactive state as computed refs
export const wsConnected = computed(() => connected.value);
export const wsDeviceId = computed(() => deviceId.value);
export const wsServerVersion = computed(() => serverVersion.value);

/**
 * Connection status for UI indicators.
 * @returns {'connected' | 'connecting' | 'disconnected'}
 */
export const wsConnectionStatus = computed(() => {
  if (connected.value) return "connected";
  if (connecting.value) return "connecting";
  return "disconnected";
});

// Export for debugging
export function getConnectionState() {
  return {
    connected: connected.value,
    connecting: connecting.value,
    deviceId: deviceId.value,
    serverVersion: serverVersion.value,
    socketState: socket.value?.readyState,
    handlersCount: handlers.size,
  };
}
