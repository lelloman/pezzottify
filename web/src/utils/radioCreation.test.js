import test from "node:test";
import assert from "node:assert/strict";
import { createRadioCreation } from "./radioCreation.js";

function harness(timeoutMs = 1000) {
  const states = [];
  const committed = [];
  const controller = createRadioCreation({
    onState: (state) => states.push(state),
    timeoutMs,
  });
  return {
    controller,
    states,
    committed,
    commit: (result) => committed.push(result),
  };
}
const radio = { trackIds: ["track"] };

test("creation is visible immediately and clears after success", async () => {
  const h = harness();
  let finish;
  const pending = h.controller.start(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
    h.commit,
  );
  assert.equal(h.states.at(-1).status, "creating");
  finish(radio);
  assert.deepEqual(await pending, radio.trackIds);
  assert.equal(h.states.at(-1).status, "idle");
  assert.deepEqual(h.committed, [radio]);
});

test("cancel and replacement discard late results", async () => {
  const h = harness();
  let finish;
  const old = h.controller.start(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
    h.commit,
  );
  await h.controller.start(async () => radio, h.commit);
  finish({ trackIds: ["stale"] });
  assert.deepEqual(await old, []);
  assert.deepEqual(h.committed, [radio]);
  const cancelled = h.controller.start(() => new Promise(() => {}), h.commit);
  h.controller.cancel();
  assert.deepEqual(await cancelled, []);
  assert.equal(h.states.at(-1).status, "idle");
});

test("timeouts terminate pending state and abort the request", async () => {
  const h = harness(10);
  let signal;
  await h.controller.start((value) => {
    signal = value;
    return new Promise(() => {});
  }, h.commit);
  assert.match(h.states.at(-1).message, /timed out/);
  assert.equal(signal.aborted, true);
  assert.equal(h.committed.length, 0);
});

test("empty and failed requests retain a retry action", async () => {
  for (const result of [null, { trackIds: [] }]) {
    const h = harness();
    let attempts = 0;
    await h.controller.start(async () => {
      if (attempts++ > 0) return radio;
      if (result === null) throw new Error("network");
      return result;
    }, h.commit);
    assert.equal(h.states.at(-1).status, "error");
    assert.equal(h.committed.length, 0);
    await h.controller.retry();
    assert.equal(h.states.at(-1).status, "idle");
    assert.deepEqual(h.committed, [radio]);
  }
});
