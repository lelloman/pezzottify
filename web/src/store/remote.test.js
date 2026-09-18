import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createPinia, setActivePinia, defineStore } from "pinia";

function harness(get) {
  setActivePinia(createPinia());
  const source = readFileSync(new URL("./remote.js", import.meta.url), "utf8")
    .replace(/^import [\s\S]*?;\n/gm, "")
    .replace("export const useRemoteStore", "const useRemoteStore");
  return new Function(
    "defineStore",
    "axios",
    source + "\nreturn useRemoteStore;",
  )(defineStore, { get })();
}

test("work versions follows link offsets through empty pages and forwards cancellation", async () => {
  const controller = new AbortController();
  const offsets = [];
  const remote = harness(async (url, options) => {
    assert.equal(url, "/v1/content/work/work%2Fid");
    assert.equal(options.signal, controller.signal);
    offsets.push(options.params.offset);
    return {
      data:
        options.params.offset === 0
          ? { tracks: [], has_more: true, next_offset: 100 }
          : {
              tracks: [{ track: { id: "version" } }],
              has_more: false,
              next_offset: 101,
            },
    };
  });
  assert.deepEqual(
    await remote.fetchWorkVersions("work/id", controller.signal),
    [{ id: "version" }],
  );
  assert.deepEqual(offsets, [0, 100]);
});

test("work versions rejects broken pagination instead of looping", async () => {
  const remote = harness(async () => ({
    data: { tracks: [], has_more: true, next_offset: 0 },
  }));
  await assert.rejects(
    remote.fetchWorkVersions("work"),
    /pagination did not advance/,
  );
});
