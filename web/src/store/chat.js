/**
 * Chat Store
 *
 * Manages AI chat state:
 * - Message history
 * - LLM provider configuration
 * - Tool execution (MCP + UI tools)
 * - Streaming responses
 */

import { defineStore } from 'pinia';
import { ref, computed, watch } from 'vue';
import { streamChat, getProvider, getProviderIds } from '../services/llm';
import { mcpClient } from '../services/mcp';
import { uiTools } from '../services/uiTools';

const CONFIG_KEY = 'ai_chat_config';
const SYSTEM_PROMPT = `You are a helpful AI assistant integrated into Pezzottify, a music streaming application.

You have access to tools that let you:
- Search and browse the music catalog (via MCP tools like catalog.search, catalog.get)
- Control music playback (play, pause, skip, queue tracks)
- Navigate the app (go to albums, artists, playlists)
- Manage user content (like/unlike, create playlists, add to playlists)
- View and change settings

IMPORTANT - ID handling rules:
1. catalog.search returns three arrays: "artists", "albums", and "tracks". Each has items with "id" and "name".
2. To add music to a playlist, you MUST use track IDs from the "tracks" array. Album and artist IDs will NOT work.
3. To get tracks from an artist or album, use catalog.get with the artist/album ID to get detailed info including track IDs.
4. ui.createPlaylist returns a "playlistId" (UUID). You MUST use this ID for ui.addToPlaylist, NOT the playlist name.
5. Never make up or guess IDs - always use IDs returned by tools.

When the user asks about music or wants to play something, use the appropriate tools.
When showing search results or content, be concise but informative.
Always prefer using tools over asking the user to do things manually.`;

export const useChatStore = defineStore('chat', () => {
  // ============================================================================
  // STATE
  // ============================================================================

  // Message history: { id, role, content, toolCalls?, toolResults? }
  const messages = ref([]);

  // Loading state
  const isLoading = ref(false);

  // Panel open state
  const isOpen = ref(false);

  // Current streaming text (for display during streaming)
  const streamingText = ref('');

  // Error state
  const error = ref(null);

  // Provider configuration
  const config = ref({
    provider: 'anthropic',
    apiKey: '',
    model: '',
    baseUrl: '', // For Ollama
  });

  // ============================================================================
  // COMPUTED
  // ============================================================================

  const isConfigured = computed(() => {
    const provider = getProvider(config.value.provider);
    if (!provider) return false;

    if (provider.requiresApiKey && !config.value.apiKey) return false;
    if (provider.requiresBaseUrl && !config.value.baseUrl) return false;

    return true;
  });

  const currentProvider = computed(() => getProvider(config.value.provider));

  const availableProviders = computed(() => {
    return getProviderIds().map(id => ({
      id,
      ...getProvider(id),
    }));
  });

  // ============================================================================
  // PERSISTENCE
  // ============================================================================

  // Load config from localStorage
  const savedConfig = localStorage.getItem(CONFIG_KEY);
  if (savedConfig) {
    try {
      const parsed = JSON.parse(savedConfig);
      config.value = { ...config.value, ...parsed };
    } catch (e) {
      console.warn('Failed to parse saved chat config:', e);
    }
  }

  // Save config to localStorage when it changes
  watch(config, (newConfig) => {
    localStorage.setItem(CONFIG_KEY, JSON.stringify(newConfig));
  }, { deep: true });

  // ============================================================================
  // TOOL HELPERS
  // ============================================================================

  /**
   * Get all available tools (MCP + UI)
   */
  function getAllTools() {
    const mcpTools = mcpClient.getTools();
    const uiToolsList = uiTools.getTools();
    return [...mcpTools, ...uiToolsList];
  }

  /**
   * Execute a tool call
   */
  async function executeTool(name, args) {
    // Check if it's a UI tool
    if (uiTools.isUiTool(name)) {
      return await uiTools.callTool(name, args);
    }

    // Otherwise it's an MCP tool
    try {
      const result = await mcpClient.callTool(name, args);
      return result;
    } catch (e) {
      return { error: e.message };
    }
  }

  // ============================================================================
  // CHAT METHODS
  // ============================================================================

  /**
   * Send a message and get a response
   */
  async function sendMessage(userMessage) {
    if (!userMessage.trim() || isLoading.value) return;

    error.value = null;
    streamingText.value = '';

    // Add user message
    const userMsg = {
      id: `msg_${Date.now()}_user`,
      role: 'user',
      content: userMessage.trim(),
    };
    messages.value.push(userMsg);

    isLoading.value = true;

    try {
      // Ensure MCP is connected
      if (!mcpClient.isConnected.value) {
        try {
          await mcpClient.connect();
        } catch (e) {
          console.warn('MCP connection failed:', e);
          // Continue without MCP tools
        }
      }

      // Get all available tools
      const tools = getAllTools();

      // Build messages for LLM (add system prompt)
      const llmMessages = [
        { role: 'user', content: SYSTEM_PROMPT },
        { role: 'assistant', content: 'I understand. I\'m ready to help you with music playback, searching, and managing your library. What would you like to do?' },
        ...messages.value,
      ];

      // Stream the response
      let assistantContent = '';
      let toolCalls = [];

      for await (const event of streamChat(config.value.provider, config.value, llmMessages, tools)) {
        if (event.type === 'text') {
          assistantContent += event.content;
          streamingText.value = assistantContent;
        } else if (event.type === 'tool_use') {
          toolCalls.push({
            id: event.id,
            name: event.name,
            input: event.input,
          });
        } else if (event.type === 'error') {
          throw new Error(event.message);
        }
      }

      // Add assistant message
      const assistantMsg = {
        id: `msg_${Date.now()}_assistant`,
        role: 'assistant',
        content: assistantContent,
        toolCalls: toolCalls.length > 0 ? toolCalls : undefined,
      };
      messages.value.push(assistantMsg);
      streamingText.value = '';

      // If there are tool calls, execute them and continue
      if (toolCalls.length > 0) {
        await handleToolCalls(toolCalls);
      }

    } catch (e) {
      console.error('Chat error:', e);
      error.value = e.message;
      streamingText.value = '';
    } finally {
      isLoading.value = false;
    }
  }

  /**
   * Handle tool calls and get follow-up response
   */
  async function handleToolCalls(toolCalls) {
    // Execute all tool calls
    const results = [];
    for (const tc of toolCalls) {
      const result = await executeTool(tc.name, tc.input);
      results.push({
        id: `result_${Date.now()}_${tc.id}`,
        role: 'tool',
        toolCallId: tc.id,
        toolName: tc.name,
        content: typeof result === 'string' ? result : JSON.stringify(result),
      });
    }

    // Add tool results to messages
    messages.value.push(...results);

    // Get follow-up response from LLM
    isLoading.value = true;
    streamingText.value = '';

    try {
      const tools = getAllTools();
      const llmMessages = [
        { role: 'user', content: SYSTEM_PROMPT },
        { role: 'assistant', content: 'I understand. I\'m ready to help you with music playback, searching, and managing your library. What would you like to do?' },
        ...messages.value,
      ];

      let assistantContent = '';
      let newToolCalls = [];

      for await (const event of streamChat(config.value.provider, config.value, llmMessages, tools)) {
        if (event.type === 'text') {
          assistantContent += event.content;
          streamingText.value = assistantContent;
        } else if (event.type === 'tool_use') {
          newToolCalls.push({
            id: event.id,
            name: event.name,
            input: event.input,
          });
        } else if (event.type === 'error') {
          throw new Error(event.message);
        }
      }

      // Add assistant message
      const assistantMsg = {
        id: `msg_${Date.now()}_assistant_followup`,
        role: 'assistant',
        content: assistantContent,
        toolCalls: newToolCalls.length > 0 ? newToolCalls : undefined,
      };
      messages.value.push(assistantMsg);
      streamingText.value = '';

      // Recursively handle more tool calls (with depth limit)
      if (newToolCalls.length > 0 && messages.value.length < 50) {
        await handleToolCalls(newToolCalls);
      }

    } catch (e) {
      console.error('Tool follow-up error:', e);
      error.value = e.message;
      streamingText.value = '';
    } finally {
      isLoading.value = false;
    }
  }

  /**
   * Clear chat history
   */
  function clearHistory() {
    messages.value = [];
    error.value = null;
    streamingText.value = '';
  }

  /**
   * Update configuration
   */
  function setConfig(newConfig) {
    config.value = { ...config.value, ...newConfig };
  }

  /**
   * Toggle panel open/closed
   */
  function toggle() {
    isOpen.value = !isOpen.value;
  }

  /**
   * Open the panel
   */
  function open() {
    isOpen.value = true;
  }

  /**
   * Close the panel
   */
  function close() {
    isOpen.value = false;
  }

  // ============================================================================
  // RETURN
  // ============================================================================

  return {
    // State
    messages,
    isLoading,
    isOpen,
    streamingText,
    error,
    config,

    // Computed
    isConfigured,
    currentProvider,
    availableProviders,

    // Methods
    sendMessage,
    clearHistory,
    setConfig,
    toggle,
    open,
    close,
  };
});
