import test from "node:test";
import assert from "node:assert/strict";
import { createPowerClient } from "./power.js";

function reply(state, status = 200) {
  return new Response(JSON.stringify({ state }), { status, headers: { "content-type": "application/json" } });
}

test("concurrent foreground requests share one wake and wait until ready", async () => {
  const calls = [];
  let finishWake;
  const client = createPowerClient(async (url, options) => {
    calls.push([url, options.method || "GET"]);
    if (url.endsWith("status")) return reply("sleeping");
    return new Promise(resolve => { finishWake = () => resolve(reply("ready")); });
  });
  const a = client.ensureReady();
  const b = client.ensureReady();
  assert.equal(a, b);
  await new Promise(resolve => setImmediate(resolve));
  assert.deepEqual(calls, [["/_power/status", "GET"], ["/_power/wake", "POST"]]);
  finishWake();
  await Promise.all([a, b]);
  await client.ensureReady();
  assert.equal(calls.length, 2);
});

test("availability checks never wake and failures remain retryable", async () => {
  const calls = [];
  const client = createPowerClient(async url => {
    calls.push(url);
    return url.endsWith("status") ? reply("sleeping") : reply("sleeping", 503);
  });
  assert.equal(await client.status(), "sleeping");
  assert.equal(calls.length, 1);
  await assert.rejects(client.ensureReady(), /still waking/);
  await assert.rejects(client.ensureReady(), /still waking/);
  assert.equal(calls.filter(url => url.endsWith("wake")).length, 2);
});

test("clients can roll out before the gateway without breaking the old server", async () => {
  let calls = 0;
  const client = createPowerClient(async () => {
    calls++;
    return new Response("<html>SPA</html>", { headers: { "content-type": "text/html" } });
  });
  await client.ensureReady();
  assert.equal(await client.status(), "ready");
  assert.equal(calls, 1);
});
