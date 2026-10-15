import test from "node:test";
import assert from "node:assert/strict";

const sockets = [];
const listeners = new Map();
globalThis.window = {
  location: { protocol: "https:", host: "test" },
  addEventListener: (name, handler) => listeners.set(name, handler),
};
globalThis.WebSocket = class {
  static OPEN = 1;
  readyState = 0;
  constructor(url) { this.url = url; sockets.push(this); }
  close(code = 1000) { this.readyState = 3; this.onclose?.({ code }); }
  confirm() {
    this.readyState = 1;
    this.onmessage({ data: JSON.stringify({ type: "connected", payload: { device_id: 1 } }) });
  }
};
const { connect, disconnect, wsConnected } = await import("./websocket.js");
const flush = () => new Promise((resolve) => setImmediate(resolve));
const reply = (state) => new Response(JSON.stringify({ state }), {
  headers: { "content-type": "application/json" },
});

for (const code of [4001, 1006, 1000]) {
  test(`close ${code} waits passively through sleep and reconnects when ready`, async (t) => {
    t.mock.timers.enable({ apis: ["setTimeout"] });
    const calls = [];
    let state = "sleeping";
    globalThis.fetch = async (url) => { calls.push(url); return reply(state); };
    try {
      connect();
      const original = sockets.at(-1);
      original.confirm();
      original.close(code);
      t.mock.timers.tick(5000); await flush();
      assert.equal(sockets.at(-1), original);
      assert.equal(wsConnected.value, false);
      state = "ready";
      t.mock.timers.tick(5000); await flush();
      const replacement = sockets.at(-1);
      assert.notEqual(replacement, original);
      assert.match(replacement.url, /wake=passive$/);
      replacement.confirm();
      assert.equal(wsConnected.value, true);
      original.onclose({ code: 1000 });
      assert.equal(wsConnected.value, true);
      assert.deepEqual(calls, ["/_power/status", "/_power/status"]);
    } finally { disconnect(); }
  });
}

test("logout during an availability check cannot reconnect", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let finish;
  globalThis.fetch = () => new Promise((resolve) => { finish = resolve; });
  connect(); sockets.at(-1).close(4001);
  t.mock.timers.tick(5000); await flush();
  const count = sockets.length;
  disconnect();
  finish(reply("ready")); await flush();
  t.mock.timers.tick(60000); await flush();
  assert.equal(sockets.length, count);
});

test("availability failure retries, and focusing the tab reconnects immediately", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  globalThis.fetch = async () => { throw new Error("offline"); };
  try {
    connect(); sockets.at(-1).close(1006);
    const count = sockets.length;
    t.mock.timers.tick(5000); await flush();
    assert.equal(sockets.length, count);
    globalThis.fetch = async () => reply("ready");
    listeners.get("focus")(); await flush();
    assert.equal(sockets.length, count + 1);
    sockets.at(-1).confirm();
    assert.equal(wsConnected.value, true);
  } finally { disconnect(); }
});
