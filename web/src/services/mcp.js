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

    pendingRequests.set(id, { resolve, reject });

    // Set timeout for request
    setTimeout(() => {
      if (pendingRequests.has(id)) {
        pendingRequests.delete(id);
        reject(new Error('MCP request timeout'));
      }
    }, 30000);

    socket.send(JSON.stringify(request));
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
  if (socket && socket.readyState === WebSocket.OPEN) {
    return Promise.resolve();
  }

  if (connecting.value) {
    // Wait for existing connection attempt
    return new Promise((resolve) => {
      const check = setInterval(() => {
        if (connected.value) {
          clearInterval(check);
          resolve();
        }
      }, 100);
    });
  }

  connecting.value = true;

  return new Promise((resolve, reject) => {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const wsUrl = `${protocol}//${window.location.host}/v1/mcp`;

    socket = new WebSocket(wsUrl);

    socket.onopen = async () => {
      connected.value = true;
      connecting.value = false;

      // Initialize the connection
      try {
        await sendRequest('initialize', {
          protocolVersion: '2024-11-05',
          capabilities: {},
          clientInfo: {
            name: 'pezzottify-web-chat',
            version: '1.0.0',
          },
        });

        // Fetch and cache tools
        await refreshTools();

        resolve();
      } catch (e) {
        console.error('MCP initialization failed:', e);
        reject(e);
      }
    };

    socket.onmessage = handleMessage;

    socket.onclose = () => {
      connected.value = false;
      connecting.value = false;
      socket = null;
      cachedTools.value = [];
    };

    socket.onerror = (error) => {
      connecting.value = false;
      reject(error);
    };
  });
}

/**
 * Disconnect from the MCP server
 */
export function disconnect() {
  if (socket) {
    socket.close();
    socket = null;
  }
  connected.value = false;
  connecting.value = false;
  cachedTools.value = [];
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
