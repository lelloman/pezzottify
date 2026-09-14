/**
 * MCP (Model Context Protocol) Client
 *
 * Connects to the backend's MCP server via WebSocket to:
 * - Fetch available tools
 * - Execute tool calls
 *
 * The MCP server is at /v1/mcp and uses the existing session cookie for auth.
 */

import { ref, computed } from 'vue';

// Module state
let socket = null;
let requestId = 0;
let connectionPromise = null;
let rejectConnection = null;
const pendingRequests = new Map();
const cachedTools = ref([]);
const connected = ref(false);
const connecting = ref(false);

/**
 * Generate a unique request ID
 */
function nextRequestId() {
  return `req_${++requestId}`;
}

/**
 * Send a JSON-RPC request and wait for response
 */
function sendRequest(method, params = {}) {
  return new Promise((resolve, reject) => {
    if (!socket || socket.readyState !== WebSocket.OPEN) {
      reject(new Error('MCP not connected'));
      return;
    }

    const id = nextRequestId();
    const request = {
      jsonrpc: '2.0',
      id,
      method,
      params,
    };

    let timer;
    pendingRequests.set(id, {
      resolve: value => { clearTimeout(timer); resolve(value); },
      reject: error => { clearTimeout(timer); reject(error); },
    });

    // Set timeout for request
    timer = setTimeout(() => {
      if (pendingRequests.has(id)) {
        pendingRequests.delete(id);
        reject(new Error('MCP request timeout'));
      }
    }, 30000);

    try { socket.send(JSON.stringify(request)); }
    catch (error) {
      pendingRequests.get(id)?.reject(error);
      pendingRequests.delete(id);
    }
  });
}

/**
 * Handle incoming WebSocket message
 */
function handleMessage(event) {
  try {
    const message = JSON.parse(event.data);

    // Handle JSON-RPC response
    if (message.id && pendingRequests.has(message.id)) {
      const { resolve, reject } = pendingRequests.get(message.id);
      pendingRequests.delete(message.id);

      if (message.error) {
        reject(new Error(message.error.message || 'MCP error'));
      } else {
        resolve(message.result);
      }
    }
  } catch (e) {
    console.error('Failed to parse MCP message:', e);
  }
}

/**
 * Connect to the MCP server
 */
export function connect() {
  if (connectionPromise) return connectionPromise;
  if (connected.value) return Promise.resolve();
  connecting.value = true;
  let ws;
  try {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    ws = new WebSocket(`${protocol}//${window.location.host}/v1/mcp`);
  } catch (error) {
    connecting.value = false;
    return Promise.reject(error);
  }
  socket = ws;
  connectionPromise = new Promise((resolve, reject) => {
    let settled = false;
    const timer = setTimeout(() => fail(new Error('MCP connection timeout')), 30000);
    const finish = error => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      if (error) reject(error); else resolve();
    };
    rejectConnection = finish;
    const fail = error => {
      finish(error);
      if (socket !== ws) return;
      disconnect();
    };
    ws.onopen = async () => {
      if (socket !== ws) return;
      try {
        await sendRequest('initialize', {
          protocolVersion: '2024-11-05', capabilities: {},
          clientInfo: { name: 'pezzottify-web-chat', version: '1.0.0' },
        });
        if (socket !== ws) return;
        const result = await sendRequest('tools/list');
        if (socket !== ws) return;
        cachedTools.value = result.tools || [];
        connected.value = true;
        connecting.value = false;
        connectionPromise = null;
        rejectConnection = null;
        finish();
      } catch (error) { fail(error); }
    };
    ws.onmessage = event => { if (socket === ws) handleMessage(event); };
    ws.onerror = () => fail(new Error('MCP connection failed'));
    ws.onclose = () => fail(new Error('MCP connection closed'));
  });
  return connectionPromise;
}

/**
 * Invalidate callbacks and settle every waiter before closing the old socket.
 */
export function disconnect() {
  const previous = socket;
  socket = null;
  const error = new Error('MCP disconnected');
  rejectConnection?.(error);
  rejectConnection = null;
  connectionPromise = null;
  for (const request of pendingRequests.values()) request.reject(error);
  pendingRequests.clear();
  connected.value = false;
  connecting.value = false;
  cachedTools.value = [];
  previous?.close();
}

/**
 * Refresh the cached tool list
 */
export async function refreshTools() {
  try {
    const result = await sendRequest('tools/list');
    cachedTools.value = result.tools || [];
  } catch (e) {
    console.error('Failed to fetch MCP tools:', e);
    cachedTools.value = [];
  }
}

/**
 * Get available tools (uses cached list)
 *
 * Returns tools in unified format for LLM:
 * { name, description, inputSchema }
 */
export function getTools() {
  return cachedTools.value.map(tool => ({
    name: tool.name,
    description: tool.description,
    inputSchema: tool.inputSchema,
  }));
}

/**
 * Call a tool and return the result
 */
export async function callTool(name, args) {
  const result = await sendRequest('tools/call', {
    name,
    arguments: args,
  });

  // Extract text content from MCP response
  if (result.content && Array.isArray(result.content)) {
    const textParts = result.content
      .filter(c => c.type === 'text')
      .map(c => c.text);
    return textParts.join('\n');
  }

  return JSON.stringify(result);
}

/**
 * Check if connected
 */
export const isConnected = computed(() => connected.value);

/**
 * Check if connecting
 */
export const isConnecting = computed(() => connecting.value);

/**
 * Get cached tools as reactive ref
 */
export const tools = computed(() => cachedTools.value);

// Export as object for convenience
export const mcpClient = {
  connect,
  disconnect,
  refreshTools,
  getTools,
  callTool,
  isConnected,
  isConnecting,
  tools,
};
