import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  bounded,
  bytes,
  redact,
  DiagnosticRecorder,
  indexedStorage,
  MAX_BYTES,
} from "./feedback/diagnostics.js";
const fixture = JSON.parse(
  readFileSync(
    new URL(
      "../../../docs/fixtures/assistant-diagnostics-v1.json",
      import.meta.url,
    ),
  ),
);
function storage() {
  let value;
  return {
    load: async () => structuredClone(value),
    save: async (v) => {
      value = structuredClone(v);
    },
    clear: async () => {
      value = undefined;
    },
    get value() {
      return value;
    },
  };
}
const now = () => "2026-09-15T08:00:01Z";
test("shared Android/schema fixture round trips unchanged, including Unicode", () => {
  assert.deepEqual(bounded(fixture), fixture);
  assert.ok(bytes(JSON.stringify(fixture)) > JSON.stringify(fixture).length);
});
test("recursive credentials and recognizable secrets redacted before persistence", async () => {
  const disk = storage(),
    recorder = new DiagnosticRecorder(disk, now);
  await recorder.account("server/A");
  await recorder.enable(true);
  recorder.event("t", "c", {
    kind: "tool_call",
    data: {
      nested: {
        api_key: "secret",
        headers: { foo: "private" },
        token: "private",
      },
      text: "Bearer abc123",
    },
  });
  await recorder.writes;
  const json = JSON.stringify(disk.value);
  assert.doesNotMatch(json, /secret|private|abc123/);
  assert.match(json, /REDACTED/);
  assert.equal(redact("sk-abcdefghijklmnopqrst"), "[REDACTED]");
});
test("exact UTF-8 boundary includes all metadata and evicts oldest complete turn", () => {
  const input = structuredClone(fixture),
    size = bytes(JSON.stringify(input));
  assert.deepEqual(bounded(input, size), input);
  const second = structuredClone(input.turns[0]);
  second.id = "next";
  input.turns.push(second);
  const kept = bounded(input, size);
  assert.equal(kept.turns.length, 1);
  assert.equal(kept.turns[0].id, "next");
});
test("oversized newest turn retains identity and explicit truncation marker under 2 MiB", () => {
  const input = structuredClone(fixture);
  input.turns[0].events[0].content = "🌍".repeat(MAX_BYTES);
  const result = bounded(input);
  assert.ok(bytes(JSON.stringify(result)) <= MAX_BYTES);
  assert.equal(result.turns[0].id, "turn-example");
  assert.match(JSON.stringify(result), /TRUNCATED/);
});
test("interrupted recovery and selected-turn immutable snapshots", async () => {
  const disk = storage(),
    first = new DiagnosticRecorder(disk, now);
  await first.account("server/A");
  await first.enable(true);
  first.event("t", "c", { kind: "user", content: "before" });
  await first.writes;
  const recovered = new DiagnosticRecorder(disk, now);
  await recovered.account("server/A");
  assert.equal(recovered.envelope.turns[0].status, "interrupted");
  const snapshot = recovered.snapshot("t");
  recovered.event("other", "c", { kind: "user", content: "after" });
  assert.equal(JSON.parse(snapshot).turns.length, 1);
  assert.doesNotMatch(snapshot, /after/);
});
test("clear, disable, and account switch invalidate queued writes", async () => {
  const disk = storage(),
    recorder = new DiagnosticRecorder(disk, now);
  await recorder.account("server/A");
  await recorder.enable(true);
  recorder.event("t", "c", { kind: "user", content: "private" });
  await recorder.clear();
  await recorder.writes;
  assert.equal(disk.value.envelope.turns.length, 0);
  recorder.event("t", "c", { kind: "user", content: "private" });
  await recorder.account("server/B");
  await recorder.writes;
  assert.equal(disk.value, undefined);
  assert.equal(recorder.enabled, false);
  await recorder.enable(true);
  recorder.event("t", "c", { kind: "user", content: "private" });
  await recorder.enable(false);
  await recorder.writes;
  assert.equal(disk.value, undefined);
});
test("unavailable/failed/evicted storage is recoverable and nonfatal", async () => {
  const recorder = new DiagnosticRecorder(indexedStorage(null), now);
  await recorder.account("server/A");
  await recorder.enable(true);
  assert.doesNotThrow(() =>
    recorder.event("t", "c", { kind: "user", content: "hello" }),
  );
  await recorder.writes;
  assert.match(recorder.error, /unavailable/);
  const disk = storage(),
    fresh = new DiagnosticRecorder(disk, now);
  await fresh.account("server/A");
  assert.equal(fresh.enabled, false);
});
test("event and turn counts remain within the backend structural limits", () => {
  const envelope = structuredClone(fixture);
  envelope.turns[0].events = Array.from({ length: 8193 }, () => ({
    kind: "metadata",
    timestamp: now(),
  }));
  const result = bounded(envelope);
  assert.equal(result.turns[0].events.length, 8192);
  assert.match(result.turns[0].events[0].content, /TRUNCATED/);
});
test("credential fields inside serialized tool-result JSON are redacted", () => {
  const result = redact(
    '{"nested":{"password":"private","api_key":"private"},"safe":"music"}',
  );
  assert.doesNotMatch(result, /private/);
  assert.match(result, /music/);
});

for (const [name, invalidate] of [
  ["account switch", (r) => r.account("server/B")],
  ["logout", (r) => r.account(null)],
  ["clear", (r) => r.clear()],
  ["disable", (r) => r.enable(false)],
  ["cross-tab clear", (r) => r.forget()],
  ["cross-tab account switch", (r) => r.forget(true)],
]) {
  for (const inFlight of [false, true]) {
    test(`pending consent cannot survive ${name} (clear ${inFlight ? "in flight" : "queued"})`, async () => {
      const disk = storage();
      const recorder = new DiagnosticRecorder(disk, now);
      await recorder.account("server/A");
      const saved = [];
      const save = disk.save;
      disk.save = async (value) => {
        saved.push(structuredClone(value));
        await save(value);
      };
      let release, started;
      const entered = new Promise((resolve) => {
        started = resolve;
      });
      const held = new Promise((resolve) => {
        release = resolve;
      });
      const clear = disk.clear;
      disk.clear = async () => {
        started();
        await held;
        await clear();
      };
      const enabling = recorder.enable(true);
      if (inFlight) await entered;
      const invalidating = invalidate(recorder);
      release();
      await Promise.all([enabling, invalidating]);
      await recorder.writes;
      assert.equal(recorder.enabled, false);
      assert.equal(disk.value, undefined);
      assert.deepEqual(saved, []);
      assert.throws(() => recorder.snapshot("t"), /disabled/);
    });
  }
}
