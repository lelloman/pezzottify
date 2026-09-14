import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { setImmediate } from 'node:timers';
import { createPinia, setActivePinia, defineStore } from 'pinia';
import { ref, computed, watch } from 'vue';
import { ChatLifecycle, runToolLoop, requiresConfirmation } from './chatLifecycle.js';

// Load the real store with isolated provider/transport dependencies. No model or
// server calls, and no importing the application's global playback/auth stores.
function storeWith({ streamChat, quickPrompt = async () => 'en', confirm = () => false } = {}) {
  setActivePinia(createPinia());
  globalThis.localStorage = { getItem: () => null, setItem() {}, removeItem() {} };
  globalThis.window = { confirm };
  const uiCalls = [];
  const mcp = { isConnected: { value: true }, connect: async () => {}, disconnect() {}, getTools: () => [] };
  const uiTools = { getTools: () => [], isUiTool: () => true, callTool: async (...args) => { uiCalls.push(args); return 'ok'; } };
  const source = readFileSync(new URL('../store/chat.js', import.meta.url), 'utf8')
    .replace(/^import .*;\n/gm, '')
    .replace('export const useChatStore', 'const useChatStore');
  const factory = new Function('defineStore', 'ref', 'computed', 'watch', 'streamChat', 'quickPrompt',
    'getProvider', 'getProviderIds', 'mcpClient', 'uiTools', 'LANGUAGES', 'getLanguage', 'buildDetectionPrompt',
    'ChatLifecycle', 'runToolLoop', 'requiresConfirmation', source + '\nreturn useChatStore;');
  const useStore = factory(defineStore, ref, computed, watch, streamChat, quickPrompt,
    () => ({}), () => [], mcp, uiTools, [], code => ({ code, name: 'English' }), text => text,
    ChatLifecycle, runToolLoop, requiresConfirmation);
  return { store: useStore(), uiCalls };
}

test('clear during detection suppresses stale output and permits a new turn', async () => {
  let finish;
  let calls = 0;
  const { store } = storeWith({
    quickPrompt: () => new Promise(resolve => { finish = resolve; }),
    streamChat: async function* () { calls++; yield { type: 'text', content: 'new response' }; },
  });
  const old = store.sendMessage('old');
  assert.equal(store.isLoading, true);
  store.clearHistory();
  finish('en');
  await old;
  assert.equal(calls, 0);
  assert.deepEqual(store.messages, []);
  assert.equal(store.isLoading, false);
  store.setLanguage('en');
  await store.sendMessage('new');
  assert.equal(store.messages.at(-1).content, 'new response');
});

test('declining an AI deletion does not call the UI tool', async () => {
  let round = 0;
  const { store, uiCalls } = storeWith({
    streamChat: async function* () {
      if (round++ === 0) yield { type: 'tool_use', id: 'delete', name: 'ui.deletePlaylist', input: { playlistId: 'A' } };
      else yield { type: 'text', content: 'Cancelled' };
    },
  });
  store.setLanguage('en');
  await store.sendMessage('delete');
  assert.deepEqual(uiCalls, []);
  assert.match(store.messages.find(m => m.role === 'tool').content, /declined/);
});

test('session reset aborts the provider signal and clears private state', async () => {
  let finish;
  let signal;
  const { store } = storeWith({
    streamChat: async function* (provider, config) {
      signal = config.signal;
      await new Promise(resolve => { finish = resolve; });
      yield { type: 'text', content: 'secret' };
    },
  });
  store.setLanguage('en');
  const send = store.sendMessage('private');
  store.resetSession();
  assert.equal(signal.aborted, true);
  finish();
  await send;
  assert.deepEqual(store.messages, []);
  assert.equal(store.streamingText, '');
  assert.equal(store.config.apiKey, '');
});

test('clear during compaction cannot restore the previous summary', async () => {
  let finish;
  const { store } = storeWith({
    quickPrompt: () => new Promise(resolve => { finish = resolve; }),
    streamChat: async function* () { yield { type: 'text', content: 'done' }; },
  });
  store.setLanguage('en');
  store.messages = Array.from({ length: 4 }, (_, i) => ({ id: String(i), role: 'user', content: 'x'.repeat(10000) }));
  await store.sendMessage('summarize');
  assert.equal(store.isCompacting, true);
  store.clearHistory();
  finish('old private summary');
  await new Promise(resolve => setImmediate(resolve));
  assert.deepEqual(store.messages, []);
  assert.equal(store.contextSummary, null);
  assert.equal(store.isCompacting, false);
});
