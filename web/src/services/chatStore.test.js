import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { setTimeout as delay } from 'node:timers/promises';
import { createPinia, setActivePinia, defineStore } from 'pinia';
import { ref, shallowRef, computed, watch } from 'vue';
import { AssistantSession } from '@lelloman/simple-assistant';
import { useAssistant } from '@lelloman/simple-assistant-vue';
import { assistantModes, requiresConfirmation, ASSISTANT_PROMPT } from './assistantModes.js';
const wasmDirectory = new URL('../wasm/', import.meta.resolve('@lelloman/simple-assistant'));
const { initSync, AssistantEngine } = await import(new URL('assistant_wasm.js', wasmDirectory));
initSync({ module: readFileSync(new URL('assistant_wasm_bg.wasm', wasmDirectory)) });
function storeWith({ stream, connect = async () => {}, confirm = () => false } = {}) {
  setActivePinia(createPinia());
  globalThis.localStorage = { getItem: () => null, setItem() {}, removeItem() {} };
  globalThis.window = { confirm };
  const calls = [];
  const mcp = { isConnected: { value: false }, connect, disconnect() {}, getTools: () => [] };
  const tools = { getTools: () => [{ name: 'ui.deletePlaylist', inputSchema: { type: 'object' } }], isUiTool: () => true, callTool: async (...args) => { calls.push(args); return 'ok'; } };
  const sessionClass = { create: options => AssistantSession.create({ ...options, createEngine: async (config, id, archive) => new AssistantEngine(JSON.stringify(config), id, archive ? JSON.stringify(archive) : undefined) }) };
  const source = readFileSync(new URL('../store/chat.js', import.meta.url), 'utf8').replace(/^import .*;\n/gm, '').replace('export const useChatStore', 'const useChatStore');
  const factory = new Function('defineStore','ref','shallowRef','computed','watch','AssistantSession','useAssistant','createProvider','getProvider','getProviderIds','mcpClient','uiTools','LANGUAGES','getLanguage','getToolDescription','getToolResultDescription','ASSISTANT_PROMPT','assistantModes','requiresConfirmation',source+'\nreturn useChatStore;');
  const useStore = factory(defineStore,ref,shallowRef,computed,watch,sessionClass,useAssistant,()=>({stream}),()=>({}),()=>[],mcp,tools,[],code=>({code,name:'English'}),()=>'',()=>'',ASSISTANT_PROMPT,assistantModes,requiresConfirmation);
  return { store: useStore(), calls, mcp };
}
async function until(predicate) { for(let i=0;i<300;i++){if(predicate())return;await delay(5);}throw new Error('Timed out'); }
test('session reset during connection prevents initialization and model calls',async()=>{
 let finish, calls=0;const {store}=storeWith({connect:()=>new Promise(r=>{finish=r;}),stream:async function*(){calls++;yield {type:'done'};}});
 const send=store.sendMessage('old');store.resetSession();finish();await send;assert.equal(calls,0);assert.deepEqual(store.messages,[]);assert.equal(store.isLoading,false);
});
test('declined tool confirmation never executes the application tool',async()=>{
 let round=0;const {store,calls}=storeWith({stream:async function*(){if(round++===0)yield {type:'tool_use',id:'a',name:'ui.deletePlaylist',input:{playlistId:'A'}};else yield {type:'text',content:'cancelled'};yield {type:'done'};}});
 store.setLanguage('en');await store.sendMessage('delete');await until(()=>!store.isLoading);assert.deepEqual(calls,[]);assert.match(store.messages.find(m=>m.role==='tool').content,/declined/);store.resetSession();
});
test('logout aborts real engine work and clears credentials',async()=>{
 let finish, signal;const {store}=storeWith({stream:async function*(_request,s){signal=s;await new Promise(r=>{finish=r;});yield {type:'text',content:'secret'};yield {type:'done'};}});
 store.setLanguage('en');await store.sendMessage('private');await until(()=>finish);store.resetSession();assert.equal(signal.aborted,true);finish();await delay(20);assert.deepEqual(store.messages,[]);assert.equal(store.config.apiKey,'');
});
test('mode definitions preserve host confirmation requirements',()=>{
 assert.equal(requiresConfirmation('ui.deletePlaylist'),true);assert.equal(requiresConfirmation('catalog.search'),false);
 const root=assistantModes([{name:'ui.deletePlaylist'},{name:'ui.play'}]);assert.deepEqual(root.children.find(m=>m.id==='help').toolIds,[]);assert.deepEqual(root.children.find(m=>m.id==='playlists').toolIds,['ui.deletePlaylist']);
});

test('reconnect refreshes available tools and retains transcript', async () => {
 let connections=0; const requests=[];
 const {store,mcp}=storeWith({connect:async()=>{connections++;},stream:async function*(request){requests.push(request);yield {type:'text',content:'ok'};yield {type:'done'};}});
 store.setLanguage('en'); await store.sendMessage('first'); await until(()=>!store.isLoading);
 mcp.getTools=()=>[{name:'catalog.search',inputSchema:{type:'object'}}];
 await store.sendMessage('second'); await until(()=>!store.isLoading);
 assert.equal(connections,2); assert.equal(store.messages.filter(m=>m.role==='user').length,2);
 assert.ok(requests.at(-1).tools.some(t=>t.name==='catalog.search')); store.resetSession();
});
