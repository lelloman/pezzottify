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
import { streamChat, quickPrompt, getProvider, getProviderIds } from '../services/llm';
import { mcpClient } from '../services/mcp';
import { uiTools } from '../services/uiTools';
import { LANGUAGES, getLanguage, buildDetectionPrompt } from '../services/languages';

const CONFIG_KEY = 'ai_chat_config';

// Compaction thresholds (in estimated tokens)
const HOT_ZONE_TOKENS = 4000; // Always keep last ~4K tokens verbatim
const STAGING_THRESHOLD = 4000; // Compact when staging area exceeds this

/**
 * Estimate token count for a message
 * Rough heuristic: ~4 chars per token for English, ~2-3 for other languages
 * We use 3.5 as a middle ground
 */
function estimateTokens(text) {
  if (!text) return 0;
  return Math.ceil(text.length / 3.5);
}

/**
 * Estimate tokens for a single message object
 */
function estimateMessageTokens(msg) {
  let tokens = 0;

  // Content
  tokens += estimateTokens(msg.content);

  // Tool calls (if any)
  if (msg.toolCalls) {
    for (const tc of msg.toolCalls) {
      tokens += estimateTokens(tc.name);
      tokens += estimateTokens(JSON.stringify(tc.input));
    }
  }

  // Role overhead
  tokens += 4; // role markers

  return tokens;
}

/**
 * Build the compaction prompt
 */
function buildCompactionPrompt(existingSummary, messagesToCompact, languageCode) {
  const lang = languageCode ? getLanguage(languageCode) : null;
  const langInstruction = lang
    ? `Write the summary in ${lang.name}.`
    : 'Write the summary in English.';

  const messagesText = messagesToCompact.map(msg => {
    let text = `[${msg.role}]: ${msg.content || ''}`;
    if (msg.toolCalls) {
      text += ` (used tools: ${msg.toolCalls.map(tc => tc.name).join(', ')})`;
    }
    if (msg.toolName) {
      text += ` (tool result for: ${msg.toolName})`;
    }
    return text;
  }).join('\n');

  if (existingSummary) {
    return `You are summarizing a conversation for context preservation.

EXISTING SUMMARY:
${existingSummary}

NEW MESSAGES TO ADD TO SUMMARY:
${messagesText}

Update the summary to include the key information from these new messages. Focus on:
- What tasks were completed (playlists created, tracks played, searches done)
- Any IDs that might be referenced later (playlist IDs, album IDs)
- User preferences or patterns learned
- Important decisions or choices made

Keep the summary concise (2-4 sentences max). ${langInstruction}

UPDATED SUMMARY:`;
  } else {
    return `You are summarizing a conversation for context preservation.

MESSAGES TO SUMMARIZE:
${messagesText}

Create a brief summary focusing on:
- What tasks were completed (playlists created, tracks played, searches done)
- Any IDs that might be referenced later (playlist IDs, album IDs)
- User preferences or patterns learned
- Important decisions or choices made

Keep the summary concise (2-4 sentences max). ${langInstruction}

SUMMARY:`;
  }
}

/**
 * Build the system prompt, optionally including language instruction
 */
function buildSystemPrompt(languageCode) {
  const basePrompt = `You are a helpful AI assistant integrated into Pezzottify, a music streaming application.

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

  if (languageCode) {
    const lang = getLanguage(languageCode);
    if (lang) {
      return `${basePrompt}

IMPORTANT: You MUST respond in ${lang.name} (${lang.nativeName}). All your responses should be written in ${lang.name}.`;
    }
  }

  return basePrompt;
}

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
    language: null, // null = not set (will auto-detect), string = ISO 639-1 code
    debugMode: false, // Show technical tool details vs friendly descriptions
  });

  // Whether we're currently detecting language
  const isDetectingLanguage = ref(false);

  // Context summary from compacted messages
  const contextSummary = ref(null);

  // Whether compaction is in progress
  const isCompacting = ref(false);

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

  // Current language info (null if not set)
  const currentLanguage = computed(() => {
    if (!config.value.language) return null;
    return getLanguage(config.value.language);
  });

  // List of all available languages
  const availableLanguages = computed(() => LANGUAGES);

  // Debug mode state
  const debugMode = computed(() => config.value.debugMode);

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
   * Build the full message list for LLM including system prompt and context
   */
  function buildLlmMessages() {
    const systemPrompt = buildSystemPrompt(config.value.language);
    const llmMessages = [
      { role: 'user', content: systemPrompt },
      { role: 'assistant', content: 'I understand. I\'m ready to help you with music playback, searching, and managing your library. What would you like to do?' },
    ];

    // Add context summary if we have one
    if (contextSummary.value) {
      llmMessages.push({
        role: 'user',
        content: `[Previous conversation context: ${contextSummary.value}]`,
      });
      llmMessages.push({
        role: 'assistant',
        content: 'I understand the context. How can I help you now?',
      });
    }

    // Add current messages
    llmMessages.push(...messages.value);

    return llmMessages;
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
   * Detect language from a user message
   */
  async function detectLanguage(userMessage) {
    if (!isConfigured.value) return null;

    isDetectingLanguage.value = true;
    try {
      const prompt = buildDetectionPrompt(userMessage);
      const response = await quickPrompt(config.value.provider, config.value, prompt);

      // Extract just the language code (first 2-3 chars, lowercase)
      const code = response.toLowerCase().trim().slice(0, 2);

      // Validate it's a known language
      const lang = getLanguage(code);
      if (lang) {
        return code;
      }

      // Fallback to English
      return 'en';
    } catch (e) {
      console.warn('Language detection failed:', e);
      return 'en';
    } finally {
      isDetectingLanguage.value = false;
    }
  }

  /**
   * Calculate token distribution across messages
   * Returns: { total, hotZone: { start, end, tokens }, staging: { start, end, tokens } }
   */
  function analyzeTokenDistribution() {
    const msgList = messages.value;
    if (msgList.length === 0) {
      return { total: 0, hotZone: null, staging: null };
    }

    // Calculate tokens from the end (most recent first)
    let total = 0;
    const tokenCounts = msgList.map(msg => estimateMessageTokens(msg));
    total = tokenCounts.reduce((a, b) => a + b, 0);

    // Find hot zone boundary (last HOT_ZONE_TOKENS)
    let hotZoneTokens = 0;
    let hotZoneStart = msgList.length;
    for (let i = msgList.length - 1; i >= 0; i--) {
      if (hotZoneTokens + tokenCounts[i] > HOT_ZONE_TOKENS) {
        break;
      }
      hotZoneTokens += tokenCounts[i];
      hotZoneStart = i;
    }

    // Staging area is everything before hot zone
    let stagingTokens = 0;
    for (let i = 0; i < hotZoneStart; i++) {
      stagingTokens += tokenCounts[i];
    }

    return {
      total,
      hotZone: {
        start: hotZoneStart,
        end: msgList.length,
        tokens: hotZoneTokens,
      },
      staging: {
        start: 0,
        end: hotZoneStart,
        tokens: stagingTokens,
      },
    };
  }

  /**
   * Check if compaction is needed and run it asynchronously
   */
  async function maybeCompact() {
    if (isCompacting.value || !isConfigured.value) return;

    const dist = analyzeTokenDistribution();

    // Only compact if staging area exceeds threshold
    if (!dist.staging || dist.staging.tokens < STAGING_THRESHOLD) {
      return;
    }

    console.log(`[Chat] Compaction triggered: staging=${dist.staging.tokens} tokens, hot=${dist.hotZone.tokens} tokens`);

    const messagesToCompact = messages.value.slice(dist.staging.start, dist.staging.end);
    if (messagesToCompact.length === 0) return;

    isCompacting.value = true;
    try {

      // Build compaction prompt
      const prompt = buildCompactionPrompt(
        contextSummary.value,
        messagesToCompact,
        config.value.language
      );

      // Get summary from LLM
      const summary = await quickPrompt(config.value.provider, config.value, prompt);

      // Update state
      contextSummary.value = summary.trim();

      // Remove compacted messages
      messages.value = messages.value.slice(dist.staging.end);

      console.log(`[Chat] Compacted ${messagesToCompact.length} messages. New summary: "${contextSummary.value.slice(0, 100)}..."`);
    } catch (e) {
      console.error('[Chat] Compaction failed:', e);
      // Non-fatal - we just keep the messages as-is
    } finally {
      isCompacting.value = false;
    }
  }

  /**
   * Send a message and get a response
   */
  async function sendMessage(userMessage) {
    if (!userMessage.trim() || isLoading.value || isCompacting.value) return;

    error.value = null;
    streamingText.value = '';

    // Detect language on first message if not set
    const trimmedMessage = userMessage.trim();
    if (config.value.language === null && messages.value.length === 0) {
      const detectedLang = await detectLanguage(trimmedMessage);
      if (detectedLang) {
        config.value.language = detectedLang;
      }
    }

    // Add user message
    const userMsg = {
      id: `msg_${Date.now()}_user`,
      role: 'user',
      content: trimmedMessage,
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

      // Build messages for LLM
      const llmMessages = buildLlmMessages();

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

      // Trigger async compaction after response (fire and forget)
      maybeCompact();
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
      const llmMessages = buildLlmMessages();

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
    contextSummary.value = null;
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

  /**
   * Set response language
   */
  function setLanguage(code) {
    config.value.language = code;
  }

  /**
   * Reset language (clear preference, will auto-detect on next first message)
   */
  function resetLanguage() {
    config.value.language = null;
  }

  /**
   * Toggle debug mode
   */
  function toggleDebugMode() {
    config.value.debugMode = !config.value.debugMode;
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
    isDetectingLanguage,
    isCompacting,
    contextSummary,

    // Computed
    isConfigured,
    currentProvider,
    availableProviders,
    currentLanguage,
    availableLanguages,
    debugMode,

    // Methods
    sendMessage,
    clearHistory,
    setConfig,
    toggle,
    open,
    close,
    setLanguage,
    resetLanguage,
    toggleDebugMode,
  };
});
