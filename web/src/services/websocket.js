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

// Reconnection state
let reconnectTimeout = null;
let connectionDesired = false;
let reconnectCheck = null;

function scheduleReconnect(delay = 5000) {
  if (!connectionDesired || socket.value || reconnectTimeout || reconnectCheck) return;
  reconnectTimeout = setTimeout(() => {
    reconnectTimeout = null;
    reconnectWhenReady();
  }, delay);
}

function reconnectWhenReady() {
  if (!connectionDesired || socket.value || reconnectCheck) return;
  // Availability polling must never wake an otherwise idle server.
  reconnectCheck = powerStatus()
    .then((state) => {
      if (state === "ready" && connectionDesired && !socket.value) connect();
    })
    .catch(() => {})
    .finally(() => {
      reconnectCheck = null;
      scheduleReconnect();
    });
}

onServerReady(() => {
  if (connectionDesired && !socket.value) connect();
});

if (typeof window !== "undefined") {
  window.addEventListener("online", reconnectWhenReady);
  window.addEventListener("focus", reconnectWhenReady);
}
if (typeof document !== "undefined") {
  document.addEventListener("visibilitychange", () => {
    if (!document.hidden) reconnectWhenReady();
  });
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
  if (socket.value) {
    return; // Already connected or connecting
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

  ws.onopen = () => {
    if (socket.value !== ws) return;
    console.log("[WS] Connection opened, waiting for server confirmation...");
  };

  ws.onmessage = (event) => {
    if (socket.value !== ws) return;
    try {
      const msg = JSON.parse(event.data);
      handleMessage(msg);
    } catch (e) {
      console.error("[WS] Failed to parse message:", e);
    }
  };

  ws.onclose = (event) => {
    if (socket.value !== ws) return;
    console.log("[WS] Connection closed:", event.code, event.reason);
    connected.value = false;
    connecting.value = false;
    deviceId.value = null;
    serverVersion.value = null;
    socket.value = null;

    // A normal server close is still retryable while the user is logged in.
    scheduleReconnect();
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

  // Clear any pending reconnect
  if (reconnectTimeout) {
    clearTimeout(reconnectTimeout);
    reconnectTimeout = null;
  }

  if (socket.value) {
    const previous = socket.value;
    socket.value = null;
    previous.close(1000, "Client disconnect");
    connected.value = false;
    connecting.value = false;
    deviceId.value = null;
    serverVersion.value = null;
  }
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
