import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createPinia, setActivePinia, defineStore } from "pinia";
import { computed, ref, shallowRef, watch, nextTick, reactive } from "vue";
import { createRadioCreation } from "../utils/radioCreation.js";
import {
  createRadioContinuation,
  editRadioContinuation,
  nextSnapshotBatch,
  appendRadioBatch,
} from "../utils/radioContinuation.js";

const ids = (count) => Array.from({ length: count }, (_, i) => `track-${i}`);
function harness() {
  setActivePinia(createPinia());
  globalThis.requestAnimationFrame = () => 1;
  globalThis.cancelAnimationFrame = () => {};
  const storage = new Map();
  globalThis.localStorage = {
    getItem: (key) => storage.get(key) ?? null,
    setItem: (key, value) => storage.set(key, String(value)),
  };
  const calls = { smart: 0, radio: 0, loads: 0, plays: 0 };
  const remote = {
    fetchArtistGreatestHits: async () => ids(25),
    fetchRadioTrackIds: async () => ids(10),
    fetchContinuationRecommendations: async () => {
      calls.smart++;
      return ["smart-track"];
    },
    fetchRadioContinuation: async () => {
      calls.radio++;
      return ["radio-track"];
    },
  };
  const user = reactive({
    isSmartContinuationEnabled: true,
    keepRadioOnQueueEdit: true,
  });
  const statics = {
    getTrack: (id) => ({ item: { id, name: id } }),
    waitArtistData: async () => ({ name: "Artist" }),
  };
  class LocalOutlet {
    constructor(callbacks) {
      this.callbacks = callbacks;
    }
    loadTrack() {
      calls.loads++;
    }
    play() {
      calls.plays++;
    }
    pause() {}
    stop() {}
    hasLoadedSound() {
      return true;
    }
  }
  // Same real-store harness used by chatStore.test.js: replace only imports, retaining Vue/Pinia behavior.
  const source = readFileSync(new URL("./playback.js", import.meta.url), "utf8")
    .replace(/^import [\s\S]*?;\n/gm, "")
    .replace("export const usePlaybackStore", "const usePlaybackStore");
  const factory = new Function(
    "defineStore",
    "computed",
    "ref",
    "shallowRef",
    "watch",
    "useStaticsStore",
    "LocalOutlet",
    "useRemoteStore",
    "useUserStore",
    "createRadioCreation",
    "createRadioContinuation",
    "editRadioContinuation",
    "nextSnapshotBatch",
    "appendRadioBatch",
    source + "\nreturn usePlaybackStore;",
  );
  const useStore = factory(
    defineStore,
    computed,
    ref,
    shallowRef,
    watch,
    () => statics,
    LocalOutlet,
    () => remote,
    () => user,
    createRadioCreation,
    createRadioContinuation,
    editRadioContinuation,
    nextSnapshotBatch,
    appendRadioBatch,
  );
  return { store: useStore(), calls, remote, user, storage };
}
async function flush() {
  for (let i = 0; i < 5; i++) await nextTick();
}

test("artist button starts ten, track nine extends, and no smart music follows exhaustion", async () => {
  const { store, calls } = harness();
  await store.setArtistGreatestHits("artist");
  await flush();
  assert.equal(store.currentPlaylist.tracksIds.length, 10);
  store.loadTrackIndex(8);
  await flush();
  assert.equal(store.currentPlaylist.tracksIds.length, 20);
  assert.equal(calls.loads, 2); // Initial play and explicit skip, no audio reload on append.
  store.loadTrackIndex(18);
  await flush();
  assert.equal(store.currentPlaylist.tracksIds.length, 25);
  store.loadTrackIndex(24);
  await flush();
  assert.equal(store.currentPlaylist.continuation.status, "exhausted");
  assert.equal(calls.smart, 0);
  store.stop();
});

test("queue edits apply the preference once and persist stopped continuation", async () => {
  const { store, user, calls, storage } = harness();
  await store.setArtistGreatestHits("artist");
  user.keepRadioOnQueueEdit = false;
  store.removeTrackFromPlaylist(3);
  await flush();
  user.keepRadioOnQueueEdit = true;
  store.loadTrackIndex(8);
  await flush();
  assert.equal(store.currentPlaylist.tracksIds.length, 9);
  assert.equal(store.currentPlaylist.continuation.status, "stopped");
  assert.equal(calls.smart, 0);
  const saved = JSON.parse(storage.get("playlistsHistory"));
  assert.equal(saved.at(-1).continuation.status, "stopped");
  store.stop();
});

test("radio continuation works with smart continuation disabled and only one request in flight", async () => {
  const { store, user, remote, calls } = harness();
  user.isSmartContinuationEnabled = false;
  let finish;
  remote.fetchRadioContinuation = () => {
    calls.radio++;
    return new Promise((resolve) => {
      finish = resolve;
    });
  };
  await store.setRadioFromItem("artist", "artist");
  store.loadTrackIndex(8);
  await flush();
  store.loadTrackIndex(9);
  await flush();
  assert.equal(calls.radio, 1);
  finish(["next-one", "next-two"]);
  await flush();
  assert.deepEqual(store.currentPlaylist.tracksIds.slice(-2), [
    "next-one",
    "next-two",
  ]);
  assert.equal(calls.smart, 0);
  store.stop();
});

test("replaced or paused sessions reject delayed results without restarting audio", async () => {
  for (const action of ["replace", "pause"]) {
    const { store, remote, calls } = harness();
    let finish;
    remote.fetchRadioContinuation = () =>
      new Promise((resolve) => {
        finish = resolve;
      });
    await store.setRadioFromItem("artist", "artist");
    store.loadTrackIndex(8);
    await flush();
    if (action === "replace") await store.setArtistGreatestHits("another");
    else store.pause();
    const plays = calls.plays;
    finish(["stale"]);
    await flush();
    assert.equal(store.currentPlaylist.tracksIds.includes("stale"), false);
    assert.equal(calls.plays, plays);
    store.stop();
  }
});

test("remote playback transports the snapshot and never generates on the controller", async () => {
  const { store, calls } = harness();
  const commands = [];
  store.setSessionStore({
    sendCommand: (...args) => commands.push(args),
    notifyStateChanged() {},
  });
  store.enterRemoteMode();
  await store.setArtistGreatestHits("artist");
  assert.equal(commands[0][0], "loadTrackIds");
  const payload = commands[0][1];
  assert.equal(payload.context.continuation.ordered_track_ids.length, 25);
  store.applyRemoteQueue(
    payload.trackIds.map((id) => ({ id })),
    payload.context,
  );
  await flush();
  assert.equal(store.currentPlaylist.continuation.next_index, 10);
  assert.equal(calls.radio, 0);
  assert.equal(calls.smart, 0);
  store.exitRemoteMode();
  store.setSessionStore(null);
  store.stop();
});

test("a batch arriving after the last track resumes at its first addition, unless paused", async () => {
  for (const pause of [false, true]) {
    const { store, remote, calls } = harness();
    let finish;
    remote.fetchRadioContinuation = () =>
      new Promise((resolve) => {
        finish = resolve;
      });
    await store.setRadioFromItem("track", "seed");
    store.loadTrackIndex(9);
    await flush();
    store.skipNextTrack();
    assert.equal(store.isPlaying, false);
    if (pause) store.pause();
    const plays = calls.plays;
    finish(["next-1", "next-2", "next-3"]);
    await flush();
    assert.equal(store.isPlaying, !pause);
    assert.equal(calls.plays, plays + (pause ? 0 : 1));
    if (!pause) assert.equal(store.currentTrackId, "next-1");
    store.stop();
  }
});

test("removing the playing track keeps the audio and queue index aligned", async () => {
  const { store } = harness();
  await store.setArtistGreatestHits("artist");
  store.loadTrackIndex(4);
  store.removeTrackFromPlaylist(4);
  await flush();
  assert.equal(store.currentTrackIndex, 4);
  assert.equal(store.currentTrackId, "track-5");
  assert.equal(
    store.currentPlaylist.tracksIds[store.currentTrackIndex],
    "track-5",
  );
  store.stop();
});

test("removing the last loaded radio track still honors keep-radio-going", async () => {
  const { store } = harness();
  await store.setArtistGreatestHits("artist");
  for (let i = 9; i > 0; i--) store.removeTrackFromPlaylist(i);
  store.removeTrackFromPlaylist(0);
  await flush();
  assert.equal(store.currentTrackId, "track-10");
  assert.equal(store.isPlaying, true);
  assert.deepEqual(store.currentPlaylist.tracksIds, ids(20).slice(10));
  store.stop();
});

test("all versions starts with the selected recording and ends without unrelated music", async () => {
  const { store, remote, calls } = harness();
  remote.fetchWorkVersions = async () => [
    { id: "other", availability: "available" },
    { id: "missing", availability: "unavailable" },
    { id: "selected", availability: "available" },
    { id: "other", availability: "available" },
  ];
  await store.setWorkVersions("work", "selected", "Composition");
  assert.deepEqual(store.currentPlaylist.tracksIds, ["selected", "other"]);
  assert.equal(store.currentPlaylist.context.source, "work_versions");
  assert.equal(store.currentPlaylist.continuation.status, "exhausted");
  store.loadTrackIndex(1);
  await flush();
  assert.equal(calls.smart, 0);
  assert.equal(calls.radio, 0);
  store.stop();
});

test("all versions respects proxy playback and preserves the queue on an empty result", async () => {
  const { store, remote, user } = harness();
  remote.fetchWorkVersions = async () => [
    { id: "remote", availability: "unavailable" },
  ];
  user.isProxyModeEnabled = true;
  await store.setWorkVersions("work", "remote", "Composition");
  assert.deepEqual(store.currentPlaylist.tracksIds, ["remote"]);
  user.isProxyModeEnabled = false;
  await store.setWorkVersions("work", "remote", "Composition");
  assert.equal(store.radioCreationState.status, "error");
  assert.deepEqual(store.currentPlaylist.tracksIds, ["remote"]);
  store.stop();
});
