import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createPinia, setActivePinia, defineStore } from "pinia";
import { reactive, computed } from "vue";

function harness() {
  setActivePinia(createPinia());
  const storage = new Map();
  globalThis.localStorage = {
    getItem: (key) => storage.get(key) ?? null,
    setItem: (key, value) => storage.set(key, value),
    removeItem: (key) => storage.delete(key),
  };
  const requests = [];
  const fetchItem = (id) =>
    new Promise((resolve, reject) => requests.push({ id, resolve, reject }));
  const remote = {
    fetchResolvedTrack: fetchItem,
    fetchResolvedAlbum: fetchItem,
    fetchArtist: fetchItem,
  };
  // Exercise the real store with only remote IO replaced, as in playback.test.js.
  const source = readFileSync(new URL("./statics.js", import.meta.url), "utf8")
    .replace(/^import .*;\n/gm, "")
    .replace("export const useStaticsStore", "const useStaticsStore");
  const useStore = new Function(
    "defineStore",
    "reactive",
    "useRemoteStore",
    source + "\nreturn useStaticsStore;",
  )(defineStore, reactive, () => remote);
  return { store: useStore(), requests, storage };
}
const flush = async () => {
  for (let i = 0; i < 8; i++) await Promise.resolve();
};
const track = (id) => ({ id, artists_ids: [], duration: 1000 });

test("90 staggered track loads remain 90 requests across repeated duration renders", async () => {
  const { store, requests } = harness();
  const ids = Array.from({ length: 90 }, (_, i) => String(i));
  const duration = computed(() =>
    ids.reduce((sum, id) => sum + (store.getTrack(id).item?.duration || 0), 0),
  );
  assert.equal(duration.value, 0);
  const waiters = ids.map((id) => store.waitTrackData(id));
  for (const id of ids) store.getTrack(id); // mounted rows share the requests
  await flush();
  assert.equal(requests.length, 90);
  for (let i = 0; i < ids.length; i++) {
    requests[i].resolve(track(ids[i]));
    await flush();
    assert.equal(duration.value, (i + 1) * 1000);
    assert.equal(requests.length, 90);
  }
  assert.equal((await Promise.all(waiters)).length, 90);
});

test("null and rejected responses settle waiters without retries on reads", async () => {
  for (const failure of ["null", "reject"]) {
    const { store, requests } = harness();
    const ref = store.getTrack("bad");
    const rejected = assert.rejects(
      store.waitTrackData("bad"),
      /Failed to fetch item/,
    );
    await flush();
    if (failure === "null") requests[0].resolve(null);
    else requests[0].reject(new Error("offline"));
    await rejected;
    assert.equal(ref.error, "Failed to fetch item");
    for (let i = 0; i < 100; i++) assert.equal(store.getTrack("bad"), ref);
    await assert.rejects(store.waitTrackData("bad"), /Failed to fetch item/);
    assert.equal(requests.length, 1);
    store.invalidateItem("tracks", "bad");
    const retry = store.waitTrackData("bad");
    await flush();
    requests[1].resolve(track("bad"));
    await retry;
    assert.equal(ref.error, null);
    assert.equal(ref.item.id, "bad");
  }
});

test("catalog invalidations discard stale results and coalesce into one subsequent request", async () => {
  const { store, requests, storage } = harness();
  const ref = store.getTrack("changed");
  const waiter = store.waitTrackData("changed");
  await flush();
  store.invalidateItem("tracks", "changed");
  store.invalidateItem("tracks", "changed");
  assert.equal(requests.length, 1);
  requests[0].resolve({ ...track("changed"), name: "stale" });
  await flush();
  assert.equal(ref.item, null);
  assert.equal(storage.has("statics_tracks_changed"), false);
  assert.equal(requests.length, 2);
  requests[1].resolve({ ...track("changed"), name: "fresh" });
  assert.equal((await waiter).name, "fresh");
  assert.equal(store.getTrack("changed"), ref);
});

test("cached data is served immediately and refreshed only once", async () => {
  const { store, requests, storage } = harness();
  storage.set("statics_tracks_cached", JSON.stringify(track("cached")));
  assert.equal((await store.waitTrackData("cached")).id, "cached");
  for (let i = 0; i < 100; i++) store.getTrack("cached");
  await flush();
  assert.equal(requests.length, 1);
  requests[0].resolve(null);
  await flush();
  assert.equal(store.getTrack("cached").error, null);
  assert.equal(store.getTrack("cached").item.id, "cached");
});

test("bad cached JSON and storage quota failures do not block remote data", async () => {
  const { store, requests, storage } = harness();
  storage.set("statics_tracks_broken", "{bad json");
  const waiter = store.waitTrackData("broken");
  await flush();
  globalThis.localStorage.setItem = () => {
    throw new Error("quota");
  };
  requests[0].resolve(track("broken"));
  assert.equal((await waiter).id, "broken");
});
