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
const context = { source: "basic", entityType: "artist", entityId: "a1" };

test("creation is visible immediately with its context and clears after success", async () => {
  const h = harness();
  let finish;
  const pending = h.controller.start(
    context,
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
    h.commit,
  );
  assert.equal(h.states.at(-1).status, "creating");
  assert.deepEqual(h.states.at(-1).context, context);
  finish(radio);
  assert.deepEqual(await pending, radio.trackIds);
  assert.equal(h.states.at(-1), null);
  assert.deepEqual(h.committed, [radio]);
});

test("the request receives its context and can refine it while pending", async () => {
  const h = harness();
  let finish;
  let received;
  const pending = h.controller.start(
    context,
    (signal, { context: requestContext, updateContext }) => {
      received = requestContext;
      updateContext({ label: "Artist" });
      return new Promise((resolve) => {
        finish = resolve;
      });
    },
    h.commit,
  );
  assert.deepEqual(received, context);
  assert.equal(h.states.at(-1).status, "creating");
  assert.equal(h.states.at(-1).context.label, "Artist");
  assert.equal(h.states.at(-1).context.entityId, "a1");
  finish({ trackIds: [] });
  await pending;
  assert.equal(h.states.at(-1).status, "error");
  assert.equal(h.states.at(-1).context.label, "Artist");
});

test("superseded attempts cannot update the pending context", async () => {
  const h = harness();
  let staleUpdate;
  h.controller.start(
    context,
    (signal, { updateContext }) => {
      staleUpdate = updateContext;
      return new Promise(() => {});
    },
    h.commit,
  );
  h.controller.start(
    { ...context, entityId: "a2" },
    () => new Promise(() => {}),
    h.commit,
  );
  staleUpdate({ label: "Stale" });
  assert.equal(h.states.at(-1).context.entityId, "a2");
  assert.equal(h.states.at(-1).context.label, undefined);
  h.controller.cancel();
  staleUpdate({ label: "Stale" });
  assert.equal(h.states.at(-1), null);
});

test("cancel and replacement discard late results", async () => {
  const h = harness();
  let finish;
  const old = h.controller.start(
    context,
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
    h.commit,
  );
  await h.controller.start(context, async () => radio, h.commit);
  finish({ trackIds: ["stale"] });
  assert.deepEqual(await old, []);
  assert.deepEqual(h.committed, [radio]);
  const cancelled = h.controller.start(
    context,
    () => new Promise(() => {}),
    h.commit,
  );
  h.controller.cancel();
  assert.deepEqual(await cancelled, []);
  assert.equal(h.states.at(-1), null);
});

test("timeouts terminate pending state and abort the request", async () => {
  const h = harness(10);
  let signal;
  await h.controller.start(
    context,
    (value) => {
      signal = value;
      return new Promise(() => {});
    },
    h.commit,
  );
  assert.match(h.states.at(-1).message, /timed out/);
  assert.deepEqual(h.states.at(-1).context, context);
  assert.equal(signal.aborted, true);
  assert.equal(h.committed.length, 0);
});

test("empty and failed requests retain a retry action with the latest context", async () => {
  for (const result of [null, { trackIds: [] }]) {
    const h = harness();
    let attempts = 0;
    const contexts = [];
    await h.controller.start(
      context,
      async (signal, { context: requestContext, updateContext }) => {
        contexts.push(requestContext);
        if (attempts++ > 0) return radio;
        updateContext({ label: "Resolved" });
        if (result === null) throw new Error("network");
        return result;
      },
      h.commit,
    );
    assert.equal(h.states.at(-1).status, "error");
    assert.equal(h.committed.length, 0);
    const retry = h.controller.retry();
    assert.equal(h.states.at(-1).status, "creating");
    assert.equal(h.states.at(-1).context.label, "Resolved");
    await retry;
    assert.equal(contexts[1].label, "Resolved");
    assert.equal(h.states.at(-1), null);
    assert.deepEqual(h.committed, [radio]);
  }
});
