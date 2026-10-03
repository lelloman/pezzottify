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
import * as gravity from "../utils/gravity.js";

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
  const smartRequests = [];
  const remote = {
    fetchArtistGreatestHits: async () => ids(25),
    fetchRadioTrackIds: async () => ids(10),
    fetchContinuationRecommendations: async (request) => {
      calls.smart++;
      smartRequests.push(request);
      return { trackIds: [`smart-track-${calls.smart}`], diagnostics: null };
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
    waitAlbumData: async (id) => ({
      id,
      name: "Album",
      discs: [{ tracks: ["album-1", "album-2", "album-3"] }],
    }),
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
    "gravity",
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
    gravity,
  );
  return { store: useStore(), calls, remote, user, storage, smartRequests };
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

test("smart continuation anchors on user-chosen tracks and excludes its own suggestions", async () => {
  const { store, smartRequests, storage } = harness();
  store.setPlaylistFromTrackIds(["u0", "u1", "u2"], 2);
  await flush();
  assert.ok(smartRequests.length >= 1);
  assert.deepEqual(smartRequests[0], {
    context_track_ids: ["u0", "u1", "u2"],
    recent_track_ids: ["u0", "u1", "u2"],
    exclude_track_ids: ["u0", "u1", "u2"],
    count: 3,
    source_track_ids: ["u0", "u1", "u2"],
  });
  assert.ok(store.currentPlaylist.tracksIds.includes("smart-track-1"));
  assert.ok(
    store.currentPlaylist.gravity.auto_track_ids.includes("smart-track-1"),
  );
  // The follow-up request keeps the source on the user's tracks and excludes the suggestion.
  const followUp = smartRequests.at(-1);
  assert.deepEqual(followUp.source_track_ids, ["u0", "u1", "u2"]);
  assert.ok(followUp.exclude_track_ids.includes("smart-track-1"));
  const saved = JSON.parse(storage.get("playlistsHistory"));
  assert.ok(saved.at(-1).gravity.auto_track_ids.includes("smart-track-1"));
  store.stop();
});

test("removed suggestions stay excluded and manual re-adds promote them to the source", async () => {
  const { store, smartRequests } = harness();
  store.setPlaylistFromTrackIds(["u0", "u1", "u2"], 2);
  await flush();
  const autoIndex = store.currentPlaylist.tracksIds.indexOf("smart-track-1");
  assert.ok(autoIndex > 2);
  store.removeTrackFromPlaylist(autoIndex);
  await flush();
  assert.equal(
    store.currentPlaylist.tracksIds.includes("smart-track-1"),
    false,
  );
  assert.ok(
    store.currentPlaylist.gravity.auto_track_ids.includes("smart-track-1"),
  );
  store.loadTrackIndex(store.currentPlaylist.tracksIds.length - 1);
  await flush();
  assert.ok(smartRequests.at(-1).exclude_track_ids.includes("smart-track-1"));
  assert.equal(
    smartRequests.at(-1).source_track_ids.includes("smart-track-1"),
    false,
  );
  store.addTracksToPlaylist(["smart-track-1"]);
  await flush();
  assert.equal(
    store.currentPlaylist.gravity.auto_track_ids.includes("smart-track-1"),
    false,
  );
  store.loadTrackIndex(store.currentPlaylist.tracksIds.length - 1);
  await flush();
  assert.ok(smartRequests.at(-1).source_track_ids.includes("smart-track-1"));
  store.stop();
});

test("editing an album keeps gravity through the mix conversion and radio has none", async () => {
  const { store, calls } = harness();
  await store.setAlbumId("album");
  await flush();
  assert.equal(store.currentPlaylist.type, store.PLAYBACK_CONTEXTS.album);
  store.loadTrackIndex(2);
  await flush();
  assert.ok(calls.smart >= 1);
  assert.ok(
    store.currentPlaylist.gravity.auto_track_ids.includes("smart-track-1"),
  );
  store.addTracksToPlaylist(["x"]);
  await flush();
  assert.equal(store.currentPlaylist.type, store.PLAYBACK_CONTEXTS.userMix);
  assert.ok(
    store.currentPlaylist.gravity.auto_track_ids.includes("smart-track-1"),
  );
  assert.ok(store.currentPlaylist.tracksIds.includes("x"));
  await store.setArtistGreatestHits("artist");
  await flush();
  assert.equal(store.currentPlaylist.gravity, null);
  assert.equal(store.currentGravity, null);
  store.stop();
});

test("gravity travels in the queue context and remote controllers send setGravity commands", async () => {
  const { store } = harness();
  const commands = [];
  store.setSessionStore({
    sendCommand: (...args) => commands.push(args),
    notifyStateChanged() {},
    notifyQueueChanged() {},
  });
  store.setPlaylistFromTrackIds(["u0", "u1", "u2"], 2);
  await flush();
  const context = store.snapshotQueueContext();
  assert.ok(context.gravity.auto_track_ids.includes("smart-track-1"));
  const queue = store.snapshotQueue();
  store.enterRemoteMode();
  store.applyRemoteQueue(queue, context);
  await flush();
  assert.deepEqual(
    store.currentPlaylist.gravity.auto_track_ids,
    context.gravity.auto_track_ids,
  );
  assert.equal(store.currentPlaylist.context.gravity, undefined);
  await store.setGravityDestination(
    { entity_type: "artist", entity_id: "a1" },
    7,
  );
  const command = commands.find(([name]) => name === "setGravity");
  assert.ok(command);
  assert.equal(command[1].gravity.destination.length, 1);
  assert.equal(command[1].gravity.destination[0].entity_id, "a1");
  assert.equal(command[1].gravity.destination[0].label, "Artist");
  assert.equal(command[1].gravity.steps_total, 7);
  assert.equal(store.currentPlaylist.gravity.destination, null);
  store.exitRemoteMode();
  store.setSessionStore(null);
  store.stop();
});

test("arriving at the destination turns it into the source and stores diagnostics", async () => {
  const { store, remote, smartRequests, calls } = harness();
  remote.fetchContinuationRecommendations = async (request) => {
    calls.smart++;
    smartRequests.push(request);
    return {
      trackIds: [`smart-track-${calls.smart}`],
      diagnostics: {
        recency_weight: 0.2,
        progress: request.progress ?? null,
        namespaces: [],
      },
    };
  };
  store.setPlaylistFromTrackIds(["u0", "u1", "u2"], 0);
  await flush();
  assert.equal(calls.smart, 0);
  await store.setGravityDestination(
    { entity_type: "artist", entity_id: "a1", label: "Dest" },
    1,
  );
  assert.equal(store.currentGravity.destination[0].label, "Dest");
  store.loadTrackIndex(2);
  await flush();
  assert.deepEqual(smartRequests[0].destination, [
    { entity_type: "artist", entity_id: "a1", weight: 1 },
  ]);
  assert.equal(smartRequests[0].progress, 0);
  const gravityState = store.currentPlaylist.gravity;
  assert.equal(gravityState.destination, null);
  assert.deepEqual(gravityState.source, {
    kind: "references",
    references: [
      { entity_type: "artist", entity_id: "a1", label: "Dest", weight: 1 },
    ],
  });
  assert.equal(gravityState.last_diagnostics.recency_weight, 0.2);
  assert.ok(Number.isFinite(gravityState.last_diagnostics.at));
  assert.ok(smartRequests.length >= 2);
  assert.deepEqual(smartRequests.at(-1).source_references, [
    { entity_type: "artist", entity_id: "a1", weight: 1 },
  ]);
  assert.equal("destination" in smartRequests.at(-1), false);
  store.stop();
});

test("destination mixes combine catalog items and concepts", async () => {
  const { store } = harness();
  store.setPlaylistFromTrackIds(["u0", "u1", "u2"], 0);
  await flush();
  await store.setGravityDestination(
    { entity_type: "artist", entity_id: "a1" },
    6,
  );
  await store.addGravityDestinationComponent({
    entity_type: "concept",
    entity_id: "audioset:Jazz",
    label: "Jazz",
    weight: 0.5,
  });
  let destination = store.currentGravity.destination;
  assert.deepEqual(
    destination.map((c) => [c.entity_type, c.entity_id, c.label, c.weight]),
    [
      ["artist", "a1", "Artist", 1],
      ["concept", "audioset:Jazz", "Jazz", 0.5],
    ],
  );
  store.setGravityDestinationComponentWeight(
    { entity_type: "concept", entity_id: "audioset:Jazz" },
    2,
  );
  assert.equal(store.currentGravity.destination[1].weight, 2);
  store.removeGravityDestinationComponent({
    entity_type: "artist",
    entity_id: "a1",
  });
  destination = store.currentGravity.destination;
  assert.deepEqual(
    destination.map((c) => c.entity_id),
    ["audioset:Jazz"],
  );
  assert.equal(store.currentGravity.steps_total, 6);
  store.stop();
});
