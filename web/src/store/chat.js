import { defineStore } from 'pinia';
import { ref, shallowRef, computed, watch } from 'vue';
import { AssistantSession } from '@lelloman/simple-assistant';
import { useAssistant } from '@lelloman/simple-assistant-vue';
import { createProvider, getProvider, getProviderIds } from '@lelloman/simple-assistant-providers';
import { mcpClient } from '../services/mcp';
import { uiTools } from '../services/uiTools';
import { LANGUAGES, getLanguage } from '../services/languages';
import { getToolDescription, getToolResultDescription } from '../services/toolDescriptions';
import { ASSISTANT_PROMPT, assistantModes, requiresConfirmation } from '../services/assistantModes';

const CONFIG_KEY = 'ai_chat_config';
export const useChatStore = defineStore('chat', () => {
  const session = shallowRef(null);
  const binding = useAssistant(session);
  const isOpen = ref(false);
  const initializing = ref(false);
  const initializationError = ref(null);
  const rootMode = ref(null);
  let generation = 0;
  let initializingPromise = null;
  // Web transcripts intentionally remain in memory, including their original mode snapshots.
  let archive = null;
  const config = ref({ provider: 'anthropic', apiKey: '', model: '', baseUrl: '', language: null, debugMode: false });
  try { const saved = localStorage.getItem(CONFIG_KEY); if (saved) config.value = { ...config.value, ...JSON.parse(saved) }; } catch { /* no stored settings */ }
  watch(config, value => { try { localStorage.setItem(CONFIG_KEY, JSON.stringify(value)); } catch { /* optional preferences */ } }, { deep: true });
  watch(binding.language, language => { if (session.value) config.value.language = language; });
  const isConfigured = computed(() => {
    const provider = getProvider(config.value.provider);
    return !!provider && (!provider.requiresApiKey || !!config.value.apiKey) && (!provider.requiresBaseUrl || !!config.value.baseUrl);
  });
  async function ensureSession() {
    if (session.value && mcpClient.isConnected.value) return session.value;
    if (initializingPromise) return initializingPromise;
    const token = generation;
    initializing.value = true; initializationError.value = null;
    const promise = (async () => {
      if (!mcpClient.isConnected.value) { try { await mcpClient.connect(); } catch { /* Local tools remain usable. */ } }
      if (token !== generation) return null;
      // Rebuild the tool catalog after reconnect, retaining the saved transcript.
      if (session.value) {
        const previous = session.value; session.value = null;
        await previous.dispose();
        if (token !== generation) return null;
      }
      const tools = [...mcpClient.getTools(), ...uiTools.getTools()];
      const root = assistantModes(tools);
      const engine = await AssistantSession.create({
        config: { basePrompt: ASSISTANT_PROMPT, rootMode: root, tools },
        provider: createProvider(config.value.provider, () => config.value),
        history: { load: async () => archive, save: async value => { if (token === generation) archive = value; } },
        executeTool: async (call, signal) => {
          signal.throwIfAborted();
          if (requiresConfirmation(call.name) && !window.confirm(`Allow AI action ${call.name}?\n\n${JSON.stringify(call.input, null, 2)}`)) return { error: 'User declined; action was not executed.' };
          signal.throwIfAborted();
          if (token !== generation) throw new DOMException('Session changed', 'AbortError');
          return uiTools.isUiTool(call.name) ? uiTools.callTool(call.name, call.input) : mcpClient.callTool(call.name, call.input);
        },
      });
      if (token !== generation) { await engine.dispose(); return null; }
      if (config.value.language && !engine.state.language) engine.setLanguage(config.value.language);
      rootMode.value = root; session.value = engine; return engine;
    })();
    initializingPromise = promise;
    try { return await promise; }
    catch (error) { if (token === generation) initializationError.value = error.message; return null; }
    finally { if (token === generation) { initializing.value = false; initializingPromise = null; } }
  }
  async function sendMessage(text) {
    if (!text.trim() || !isConfigured.value || binding.isLoading.value || initializing.value) return;
    const engine = await ensureSession(); engine?.send(text);
  }
  function invalidate() {
    generation++; initializingPromise = null; initializing.value = false; initializationError.value = null;
    const old = session.value; session.value = null;
    if (old) { old.cancel(); void old.dispose().catch(error => { initializationError.value = error.message; }); }
    mcpClient.disconnect();
  }
  function clearHistory() { invalidate(); archive = null; config.value.language = null; }
  function resetSession() { clearHistory(); isOpen.value = false; config.value.apiKey = ''; localStorage.removeItem(CONFIG_KEY); }
  function setConfig(value) { clearHistory(); config.value = { ...config.value, ...value }; }
  function setLanguage(code) { config.value.language = code; session.value?.setLanguage(code); }
  const modes = computed(() => {
    const flatten = (node, path = []) => [{ id: node.id, name: node.name, label: [...path, node.name].join(' / ') }, ...(node.children || []).flatMap(child => flatten(child, [...path, node.name]))];
    return rootMode.value ? flatten(rootMode.value) : [];
  });
  return {
    ...binding, config, isOpen, isConfigured, modes,
    isLoading: computed(() => initializing.value || binding.isLoading.value),
    error: computed(() => initializationError.value || binding.error.value),
    contextSummary: computed(() => binding.state.value?.summary?.content ?? null),
    currentProvider: computed(() => getProvider(config.value.provider)),
    availableProviders: computed(() => getProviderIds().map(id => ({ id, ...getProvider(id) }))),
    currentLanguage: computed(() => config.value.language ? getLanguage(config.value.language) : null),
    availableLanguages: LANGUAGES, debugMode: computed(() => config.value.debugMode),
    sendMessage, clearHistory, resetSession, setConfig, setLanguage, resetLanguage: () => setLanguage(null),
    toggle: () => { isOpen.value = !isOpen.value; }, open: () => { isOpen.value = true; }, close: () => { isOpen.value = false; },
    toggleDebugMode: () => { config.value.debugMode = !config.value.debugMode; },
    welcome: 'Ask me anything about your music!',
    suggestions: ['Search for jazz music', 'What is currently playing?', 'Show my liked albums'],
    describeTool: getToolDescription, describeResult: getToolResultDescription,
  };
});
