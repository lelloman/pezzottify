import test from "node:test";
import assert from "node:assert/strict";
import "vue";

const sockets = [];
const listeners = new Map();
const documentListeners = new Map();
globalThis.document = {
  hidden: false,
  addEventListener: (name, handler) => documentListeners.set(name, handler),
};
function visibility(hidden) {
  document.hidden = hidden;
  documentListeners.get("visibilitychange")();
}
globalThis.window = {
  location: { protocol: "https:", host: "test" },
  addEventListener: (name, handler) => listeners.set(name, handler),
};
globalThis.WebSocket = class {
  static OPEN = 1;
  static CONNECTING = 0;
  sent = [];
  send(data) { this.sent.push(JSON.parse(data)); }
  pong() { this.onmessage({ data: JSON.stringify({ type: "pong" }) }); }
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
      t.mock.timers.tick(30000); await flush();
      assert.equal(sockets.at(-1), original);
      assert.equal(wsConnected.value, false);
      state = "ready";
      t.mock.timers.tick(30000); await flush();
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
  t.mock.timers.tick(30000); await flush();
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
    t.mock.timers.tick(30000); await flush();
    assert.equal(sockets.length, count);
    globalThis.fetch = async () => reply("ready");
    listeners.get("focus")(); await flush();
    assert.equal(sockets.length, count + 1);
    sockets.at(-1).confirm();
    assert.equal(wsConnected.value, true);
  } finally { disconnect(); }
});


test("hidden tab stops polling and reconnects immediately when visible", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const calls = [];
  globalThis.fetch = async (url) => { calls.push(url); return reply("ready"); };
  try {
    connect(); const original = sockets.at(-1); original.confirm();
    visibility(true); original.close(4001);
    t.mock.timers.tick(180000); await flush();
    listeners.get("focus")(); await flush();
    assert.deepEqual(calls, []);
    assert.equal(sockets.at(-1), original);
    visibility(false); await flush();
    assert.notEqual(sockets.at(-1), original);
    assert.deepEqual(calls, ["/_power/status"]);
  } finally { disconnect(); document.hidden = false; }
});

test("old OPEN socket without close event is replaced after failed foreground ping", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout", "Date"], now: 0 });
  globalThis.fetch = async () => reply("ready");
  try {
    connect(); const original = sockets.at(-1); original.confirm();
    visibility(true); t.mock.timers.tick(3600000); await flush();
    assert.deepEqual(original.sent, []);
    visibility(false);
    assert.equal(original.sent.at(-1).type, "ping");
    t.mock.timers.tick(10000); await flush();
    assert.notEqual(sockets.at(-1), original);
    sockets.at(-1).confirm();
    original.onclose({ code: 1006 });
    assert.equal(wsConnected.value, true);
  } finally { disconnect(); document.hidden = false; }
});

for (const opened of [false, true]) {
  test(`unfinished ${opened ? "confirmation" : "connection"} from an old tab is replaced`, async (t) => {
    t.mock.timers.enable({ apis: ["setTimeout", "Date"], now: 0 });
    globalThis.fetch = async () => reply("ready");
    try {
      connect(); const original = sockets.at(-1);
      if (opened) original.readyState = WebSocket.OPEN;
      visibility(true); t.mock.timers.tick(3600000);
      visibility(false); t.mock.timers.tick(0); await flush();
      assert.notEqual(sockets.at(-1), original);
      sockets.at(-1).confirm(); assert.equal(wsConnected.value, true);
    } finally { disconnect(); document.hidden = false; }
  });
}

test("foreground connection and confirmation attempts have a deadline", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout", "Date"], now: 0 });
  globalThis.fetch = async () => reply("ready");
  try {
    connect(); const first = sockets.at(-1);
    t.mock.timers.tick(10000); await flush();
    assert.equal(first.readyState, 3);
    t.mock.timers.tick(30000); await flush();
    const second = sockets.at(-1); assert.notEqual(second, first);
    second.readyState = WebSocket.OPEN;
    t.mock.timers.tick(10000); await flush();
    assert.equal(second.readyState, 3);
  } finally { disconnect(); }
});

test("responsive sockets stay connected; heartbeat stops in background", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  try {
    connect(); const original = sockets.at(-1); original.confirm();
    t.mock.timers.tick(30000);
    assert.equal(original.sent.length, 1); original.pong();
    visibility(true); t.mock.timers.tick(3600000);
    assert.equal(original.sent.length, 1);
    visibility(false); assert.equal(original.sent.length, 2); original.pong();
    t.mock.timers.tick(10000);
    assert.equal(sockets.at(-1), original); assert.equal(wsConnected.value, true);
  } finally { disconnect(); document.hidden = false; }
});

test("backgrounding aborts availability checks and ignores late responses", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let finish, signal;
  globalThis.fetch = (_url, options) => {
    signal = options.signal;
    return new Promise((resolve) => { finish = resolve; });
  };
  try {
    connect(); sockets.at(-1).close(4001);
    t.mock.timers.tick(30000); await flush();
    const count = sockets.length;
    visibility(true); assert.equal(signal.aborted, true);
    finish(reply("ready")); await flush();
    assert.equal(sockets.length, count);
    globalThis.fetch = async () => reply("ready");
    visibility(false); await flush(); assert.equal(sockets.length, count + 1);
  } finally { disconnect(); document.hidden = false; }
});

test("hung availability check cannot block foreground recovery", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let finish;
  globalThis.fetch = () => new Promise((resolve) => { finish = resolve; });
  try {
    connect(); sockets.at(-1).close(4001);
    t.mock.timers.tick(30000); await flush();
    t.mock.timers.tick(5000); await flush();
    globalThis.fetch = async () => reply("ready");
    listeners.get("pageshow")(); await flush();
    const replacement = sockets.at(-1);
    replacement.confirm();
    finish(reply("sleeping")); await flush();
    assert.equal(wsConnected.value, true);
    assert.equal(sockets.at(-1), replacement);
  } finally { disconnect(); }
});
