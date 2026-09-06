import assert from "node:assert/strict";
import test from "node:test";
import { abortableDelay, fetchImageBlob } from "./imageLoader.js";

const success = () => new Response("image", { status: 200 });

test("retries 5xx with exponential jitter, then returns the image", async () => {
  const delays = [];
  const randoms = [0, 0.5, 1];
  let calls = 0;
  const blob = await fetchImageBlob("/image", {
    fetchImpl: async () =>
      ++calls < 4 ? new Response(null, { status: 503 }) : success(),
    wait: async (ms) => delays.push(ms),
    random: () => randoms.shift(),
  });
  assert.equal(await blob.text(), "image");
  assert.equal(calls, 4);
  assert.deepEqual(delays, [250, 750, 2000]);
});

test("stops after three retries", async () => {
  let calls = 0;
  await assert.rejects(
    fetchImageBlob("/image", {
      fetchImpl: async () => {
        calls++;
        return new Response(null, { status: 500 });
      },
      wait: async () => {},
    }),
    /HTTP 500/,
  );
  assert.equal(calls, 4);
});

test("does not retry permanent HTTP failures", async () => {
  for (const status of [400, 401, 403, 404]) {
    let calls = 0;
    await assert.rejects(
      fetchImageBlob("/image", {
        fetchImpl: async () => {
          calls++;
          return new Response(null, { status });
        },
        wait: async () => assert.fail("unexpected retry"),
      }),
      new RegExp(`HTTP ${status}`),
    );
    assert.equal(calls, 1);
  }
});

test("retries network failures and HTTP request timeouts", async () => {
  let calls = 0;
  await fetchImageBlob("/image", {
    fetchImpl: async () => {
      if (++calls === 1) throw new TypeError("Failed to fetch");
      if (calls === 2) return new Response(null, { status: 408 });
      return success();
    },
    wait: async () => {},
  });
  assert.equal(calls, 3);
});

test("deadline covers a stalled response body and retries it", async () => {
  let calls = 0;
  const blob = await fetchImageBlob("/image", {
    timeoutMs: 5,
    fetchImpl: async (_, { signal }) => {
      if (++calls > 1) return success();
      return {
        ok: true,
        blob: () =>
          new Promise((_, reject) => {
            signal.addEventListener("abort", () => reject(signal.reason), {
              once: true,
            });
          }),
      };
    },
    wait: async () => {},
  });
  assert.equal(await blob.text(), "image");
  assert.equal(calls, 2);
});

test("cancelling an active request prevents retries", async () => {
  const controller = new AbortController();
  const pending = fetchImageBlob("/image", {
    signal: controller.signal,
    fetchImpl: (_, { signal }) =>
      new Promise((_, reject) => {
        signal.addEventListener("abort", () => reject(signal.reason), {
          once: true,
        });
      }),
    wait: async () => assert.fail("unexpected retry"),
  });
  controller.abort();
  await assert.rejects(pending, { name: "AbortError" });
});

test("cancelling during backoff prevents another request", async () => {
  const controller = new AbortController();
  let calls = 0;
  await assert.rejects(
    fetchImageBlob("/image", {
      signal: controller.signal,
      fetchImpl: async () => {
        calls++;
        return new Response(null, { status: 502 });
      },
      wait: (ms, signal) => {
        const pending = abortableDelay(ms, signal);
        controller.abort();
        return pending;
      },
    }),
    { name: "AbortError" },
  );
  assert.equal(calls, 1);
});
