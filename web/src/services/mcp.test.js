import test from 'node:test';
import assert from 'node:assert/strict';
import { connect, disconnect, isConnected, callTool } from './mcp.js';

const sockets = [];
globalThis.window = { location: { protocol: 'https:', host: 'test' } };
globalThis.WebSocket = class {
  static OPEN = 1;
  readyState = 0;
  constructor() { sockets.push(this); }
  close() { this.readyState = 3; this.onclose?.(); }
  send(data) {
    const request = JSON.parse(data);
    if (request.method !== 'tools/call') queueMicrotask(() => this.onmessage?.({
      data: JSON.stringify({ id: request.id, result: { tools: [] } }),
    }));
  }
  open() { this.readyState = 1; this.onopen(); }
};

test('concurrent connect callers share rejection and can retry', async () => {
  const first = connect();
  const second = connect();
  assert.equal(first, second);
  const results = Promise.allSettled([first, second]);
  sockets.at(-1).onerror();
  assert.deepEqual((await results).map(r => r.status), ['rejected', 'rejected']);
  const retry = connect();
  sockets.at(-1).open();
  await retry;
  assert.equal(isConnected.value, true);
  disconnect();
});

test('disconnect rejects outstanding RPCs and stale socket cannot kill reconnect', async () => {
  const first = connect();
  const old = sockets.at(-1);
  old.open();
  await first;
  const rpc = callTool('catalog.get', {});
  const rejected = assert.rejects(rpc, /disconnected/);
  disconnect();
  await rejected;
  const next = connect();
  sockets.at(-1).open();
  await next;
  old.onclose();
  assert.equal(isConnected.value, true);
  disconnect();
});
