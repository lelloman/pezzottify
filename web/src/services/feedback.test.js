import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createPinia, setActivePinia, defineStore } from "pinia";
import { ref, shallowRef } from "vue";
import { DiagnosticRecorder, bytes, redact } from "./feedback/diagnostics.js";
import { authorizationHeaderValue } from "./authorization.js";
const source = readFileSync(
  new URL("../store/feedback.js", import.meta.url),
  "utf8",
)
  .replace(/^import [\s\S]*?;\n/gm, "")
  .replace("export const useFeedbackStore", "const useFeedbackStore");
function setup(getIdToken = async () => "account-A") {
  setActivePinia(createPinia());
  globalThis.window = { location: { origin: "https://example.org" } };
  globalThis.localStorage = { getItem: () => null, setItem() {} };
  let data;
  class Recorder extends DiagnosticRecorder {
    constructor() {
      super({
        load: async () => data,
        save: async (v) => {
          data = v;
        },
        clear: async () => {
          data = null;
        },
      });
    }
  }
  const factory = new Function(
    "defineStore",
    "ref",
    "shallowRef",
    "DiagnosticRecorder",
    "bytes",
    "redact",
    "oidc",
    "getCsrfToken",
    "authorizationHeaderValue",
    source + "\nreturn useFeedbackStore;",
  );
  const store = factory(
    defineStore,
    ref,
    shallowRef,
    Recorder,
    bytes,
    redact,
    { getIdToken },
    () => null,
    authorizationHeaderValue,
  )();
  return store;
}
const form = () => ({
  kind: "bug",
  category: "assistant",
  title: "Unicode 🌍",
  description: "failure",
  conversation: true,
  allTurns: false,
  logs: false,
  technicalLogs: "",
});
test("preview/upload identity and manual idempotent retry do not collect live activity", async () => {
  const store = setup();
  await store.account("A");
  await store.setEnabled(true);
  store.record("t", "c", { kind: "user", content: "before" }, "completed");
  store.show("t");
  await store.freeze(form());
  const body = store.preview.body;
  store.record("other", "c", { kind: "user", content: "after" }, "completed");
  const sent = [];
  globalThis.fetch = async (_url, options) => {
    if (_url.pathname === "/v1/auth/session")
      return { ok: true, json: async () => ({ user_handle: "A" }) };
    sent.push(options);
    if (sent.length === 1) throw new Error("offline");
    return { ok: true, status: 201, json: async () => ({ id: "report-id" }) };
  };
  await store.send();
  assert.equal(sent.length, 1);
  assert.equal(store.preview.body, body);
  assert.equal(store.receipt, null);
  await store.send();
  assert.equal(sent.length, 2);
  assert.equal(sent[0].body, body);
  assert.equal(sent[1].body, body);
  assert.doesNotMatch(body, /after/);
  assert.equal(store.receipt, "report-id");
  assert.equal(sent[0].redirect, "error");
});
test("account change while resolving credentials never dispatches stale upload", async () => {
  let resolve;
  const store = setup(
    () =>
      new Promise((r) => {
        resolve = r;
      }),
  );
  await store.account("A");
  await store.freeze({ ...form(), conversation: false });
  let calls = 0;
  globalThis.fetch = async () => {
    calls++;
  };
  const pending = store.send();
  await store.account("B");
  resolve("old-account");
  await pending;
  assert.equal(calls, 0);
  assert.equal(store.preview, null);
  assert.equal(store.receipt, null);
});
test("clear and disable invalidate unsent snapshots; attachments require separate consent", async () => {
  const store = setup();
  await store.account("A");
  await store.setEnabled(true);
  store.record("t", "c", { kind: "user", content: "private" });
  store.show("t");
  await store.freeze({ ...form(), conversation: false });
  assert.deepEqual(JSON.parse(store.preview.body).attachments, []);
  await store.freeze(form());
  assert.equal(JSON.parse(store.preview.body).attachments[0].consent, true);
  await store.clear();
  assert.equal(store.preview, null);
  await store.freeze(form());
  assert.match(store.error, /No retained/);
  await store.setEnabled(false);
  await store.freeze(form());
  assert.match(store.error, /disabled/);
});
test("structured capacity/quota errors retain manual retry snapshot and show retry delay", async () => {
  const store = setup();
  await store.account("A");
  await store.freeze({ ...form(), conversation: false });
  const body = store.preview.body;
  globalThis.fetch = async (url) =>
    url.pathname === "/v1/auth/session"
      ? { ok: true, json: async () => ({ user_handle: "A" }) }
      : {
          ok: false,
          status: 429,
          headers: new Headers({ "Retry-After": "3600" }),
        };
  await store.send();
  assert.match(store.error, /quota.*3600/);
  assert.equal(store.preview.body, body);
});
test("verified credentials cannot retarget a report to another account or cookies", async () => {
  const store = setup();
  await store.account("A");
  await store.freeze({ ...form(), conversation: false });
  let uploads = 0;
  globalThis.fetch = async (url, options) => {
    assert.equal(options.credentials, "omit");
    if (url.pathname === "/v1/auth/session")
      return { ok: true, json: async () => ({ user_handle: "B" }) };
    uploads++;
  };
  await store.send();
  assert.equal(uploads, 0);
  assert.match(store.error, /Account changed/);
});
test("in-flight response after logout cannot restore a receipt or preview", async () => {
  const store = setup();
  await store.account("A");
  await store.freeze({ ...form(), conversation: false });
  let finish;
  globalThis.fetch = async (url) =>
    url.pathname === "/v1/auth/session"
      ? { ok: true, json: async () => ({ user_handle: "A" }) }
      : new Promise((resolve) => {
          finish = resolve;
        });
  const send = store.send();
  while (!finish) await new Promise((resolve) => setTimeout(resolve, 0));
  await store.account(null);
  finish({ ok: true, status: 201, json: async () => ({ id: "stale" }) });
  await send;
  assert.equal(store.receipt, null);
  assert.equal(store.preview, null);
});
test("events from a pre-clear turn cannot repopulate the recording", async () => {
  const store = setup();
  await store.account("A");
  await store.setEnabled(true);
  const epoch = store.epoch();
  store.record(
    "old",
    "c",
    { kind: "user", content: "private" },
    "running",
    epoch,
  );
  await store.clear();
  store.record(
    "old",
    "c",
    { kind: "assistant", content: "late private result" },
    "completed",
    epoch,
  );
  store.show("old");
  await store.freeze(form());
  assert.match(store.error, /No retained/);
});
