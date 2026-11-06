import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createPinia, setActivePinia, defineStore } from "pinia";
import { computed, ref, watch, nextTick } from "vue";

// Same real-store harness as playback.test.js: strip the imports, inject fakes.
function harness(t, { playback = {}, remote = {}, statics = {} } = {}) {
  setActivePinia(createPinia());
  const sent = [];
  let keepAlive = null;
  const ws = {
    wsConnected: ref(true),
    send: (type, payload) => sent.push({ type, payload }),
    setKeepAliveProvider: (fn) => {
      keepAlive = fn;
    },
  };
  const remoteStore = {
    fetchPlaylistData: async (id) =>
      id === "pl-1" ? { id, name: "Mix", tracks: ["p1", "p2", "p3"] } : null,
    ...remote,
  };
  const staticsStore = {
    waitAlbumData: async (id) =>
      id === "al-1"
        ? { id, discs: [{ tracks: ["a1", "a2"] }, { tracks: ["a3"] }] }
        : null,
    ...statics,
  };
  const calls = [];
  const record =
    (name) =>
    (...args) => {
      calls.push([name, ...args]);
    };
  const playbackStore = {
    mode: "local",
    currentTrackId: null,
    currentTrack: null,
    isPlaying: false,
    currentPlaylist: { tracksIds: ["x", "y"] },
    snapshotState: (queueVersion) => ({
      current_track: { id: playbackStore.currentTrackId, duration: 100 },
      queue_version: queueVersion,
      is_playing: playbackStore.isPlaying,
      position: 1,
    }),
    snapshotQueue: () => [{ id: "x", added_at: 1 }],
    snapshotQueueContext: () => ({ type: "user_mix" }),
    play: record("play"),
    pause: record("pause"),
    skipNextTrack: record("next"),
    skipPreviousTrack: record("prev"),
    seekToPercentage: record("seekToPercentage"),
    setVolume: record("setVolume"),
    setMuted: record("setMuted"),
    setAlbumId: record("setAlbumId"),
    setUserPlaylist: record("setUserPlaylist"),
    setPlaylistFromTrackIds: record("setPlaylistFromTrackIds"),
    addTracksToPlaylist: record("addTracksToPlaylist"),
    loadTrackIndex: record("loadTrackIndex"),
    removeTrackFromPlaylist: record("removeTrackFromPlaylist"),
    moveTrack: record("moveTrack"),
    applyGravity: record("applyGravity"),
    applyRemoteState: record("applyRemoteState"),
    applyRemoteQueue: record("applyRemoteQueue"),
    ...playback,
  };
  const source = readFileSync(
    new URL("./playbackSession.js", import.meta.url),
    "utf8",
  )
    .replace(/^import [\s\S]*?;\n/gm, "")
    .replace(
      "export const usePlaybackSessionStore",
      "const usePlaybackSessionStore",
    );
  const factory = new Function(
    "defineStore",
    "ref",
    "computed",
    "watch",
    "ws",
    "getDeviceName",
    "useRemoteStore",
    "useStaticsStore",
    source + "\nreturn usePlaybackSessionStore;",
  );
  const useStore = factory(
    defineStore,
    ref,
    computed,
    watch,
    ws,
    () => "Test Browser on Test OS",
    () => remoteStore,
    () => staticsStore,
  );
  const store = useStore();
  store.setPlaybackStore(playbackStore);
  // Broadcasting starts real intervals; clear them so the runner can exit.
  t.after(() => store.cleanup());
  const ofType = (type) => sent.filter((m) => m.type === type);
  return {
    store,
    sent,
    ofType,
    calls,
    playbackStore,
    ws,
    keepAlive: () => keepAlive,
  };
}

async function flush() {
  for (let i = 0; i < 5; i++) await nextTick();
  await new Promise((resolve) => setImmediate(resolve));
}

const welcome = (overrides = {}) => ({
  device_id: 1,
  devices: [
    { id: 1, name: "me", device_type: "web", is_playing: false },
    { id: 2, name: "phone", device_type: "android", is_playing: true },
  ],
  session: {
    active_devices: [
      {
        device_id: 2,
        device_name: "phone",
        state: { is_playing: true, position: 10, current_track: { id: "t" } },
        queue: [{ id: "t", added_at: 1 }],
        queue_version: 3,
        context: { type: "album" },
      },
      {
        device_id: 1,
        device_name: "me",
        state: { is_playing: false },
        queue: [],
        queue_version: 0,
      },
    ],
  },
  ...overrides,
});

test("hello carries the shared device name and welcome fills devices and states", (t) => {
  t.mock.timers.enable({ apis: ["Date"], now: 5000 });
  const { store, ofType } = harness(t);
  store.sendHello();
  assert.deepEqual(ofType("playback.hello")[0].payload, {
    device_name: "Test Browser on Test OS",
    device_type: "web",
  });
  store.handleMessage("playback.welcome", welcome());
  assert.equal(store.myDeviceId, 1);
  assert.equal(store.devices.length, 2);
  assert.equal(store.otherDevicesCount, 1);
  assert.deepEqual(Object.keys(store.otherDeviceStates), ["2"]);
  const other = store.otherDeviceStates[2];
  assert.equal(other.deviceName, "phone");
  assert.equal(other.receivedAt, 5000);
  assert.equal(other.queueVersion, 3);
  assert.deepEqual(other.queueContext, { type: "album" });
});

test("device_state updates the relayed state with a local receivedAt and ignores self", (t) => {
  t.mock.timers.enable({ apis: ["Date"], now: 1000 });
  const { store } = harness(t);
  store.handleMessage("playback.welcome", welcome());
  t.mock.timers.tick(2500);
  store.handleMessage("playback.device_state", {
    device_id: 2,
    device_name: "phone",
    state: { is_playing: false, position: 20, timestamp: 999999 },
  });
  assert.equal(store.otherDeviceStates[2].state.position, 20);
  assert.equal(store.otherDeviceStates[2].receivedAt, 3500);
  assert.equal(store.otherDeviceStates[2].queueVersion, 3);
  store.handleMessage("playback.device_state", {
    device_id: 1,
    device_name: "me",
    state: { is_playing: true },
  });
  assert.equal(store.otherDeviceStates[1], undefined);
  store.handleMessage("playback.device_stopped", { device_id: 2 });
  assert.deepEqual(store.otherDeviceStates, {});
});

test("broadcastState never sends a null track while metadata is unresolved", (t) => {
  const { store, ofType, playbackStore } = harness(t);
  playbackStore.currentTrackId = "t1";
  playbackStore.currentTrack = null;
  store.notifyPlaybackStarted();
  assert.equal(ofType("playback.state").length, 0);
  assert.equal(ofType("playback.queue_update").length, 1);
  assert.equal(store.isBroadcasting, true);
  playbackStore.currentTrack = { id: "t1", duration: 100000 };
  store.notifyStateChanged();
  const states = ofType("playback.state");
  assert.equal(states.length, 1);
  assert.equal(states[0].payload.current_track.id, "t1");
});

test("welcome re-announces state and queue together when a track is loaded", (t) => {
  const { store, ofType, playbackStore } = harness(t);
  playbackStore.currentTrackId = "t1";
  playbackStore.currentTrack = { id: "t1" };
  store.handleMessage("playback.welcome", welcome());
  assert.equal(ofType("playback.state").length, 1);
  assert.equal(ofType("playback.queue_update").length, 1);
  assert.equal(ofType("playback.queue_update")[0].payload.queue_version, 1);
  assert.equal(store.isBroadcasting, true);
});

test("notifyStopped sends the stop marker and stops broadcasting", (t) => {
  const { store, ofType, playbackStore } = harness(t);
  playbackStore.currentTrackId = "t1";
  playbackStore.currentTrack = { id: "t1" };
  store.notifyPlaybackStarted();
  store.notifyStopped();
  const last = ofType("playback.state").at(-1).payload;
  assert.equal(last.current_track, null);
  assert.equal(last.is_playing, false);
  assert.equal(store.isBroadcasting, false);
});

test("playback.error is kept as lastError and can be cleared", (t) => {
  const { store } = harness(t);
  store.handleMessage("playback.welcome", welcome());
  store.handleMessage("playback.error", {
    code: "forbidden",
    message: "Forbidden",
    context: { command: "play" },
  });
  assert.equal(store.lastError.code, "forbidden");
  assert.equal(store.lastError.command, "play");
  store.clearError();
  assert.equal(store.lastError, null);
});

test("commands run on the local player and are followed by a state broadcast", async (t) => {
  const { store, calls, ofType, playbackStore } = harness(t);
  playbackStore.currentTrackId = "t1";
  playbackStore.currentTrack = { id: "t1", duration: 200000 };
  store.handleMessage("playback.command", { command: "play", payload: {} });
  await flush();
  assert.deepEqual(calls.at(-1), ["play"]);
  assert.equal(ofType("playback.state").length, 1);

  store.handleMessage("playback.command", {
    command: "seek",
    payload: { position: 50 },
  });
  await flush();
  assert.deepEqual(calls.at(-1), ["seekToPercentage", 0.25]);

  store.handleMessage("playback.command", {
    command: "loadPlaylist",
    payload: { playlistId: "pl-1", startTrackId: "p3" },
  });
  await flush();
  const load = calls.find(([name]) => name === "setUserPlaylist");
  assert.deepEqual(load[1].tracks, ["p1", "p2", "p3"]);
  assert.equal(load[2], 2);

  store.handleMessage("playback.command", {
    command: "addAlbumToQueue",
    payload: { albumId: "al-1" },
  });
  await flush();
  assert.deepEqual(calls.at(-1), ["addTracksToPlaylist", ["a1", "a2", "a3"]]);

  store.handleMessage("playback.command", {
    command: "loadAlbum",
    payload: { albumId: "al-1", startTrackId: "a3" },
  });
  await flush();
  assert.deepEqual(calls.at(-1), ["setAlbumId", "al-1", 1, 0]);

  const before = calls.length;
  store.handleMessage("playback.command", {
    command: "setShuffle",
    payload: { enabled: true },
  });
  await flush();
  assert.equal(calls.length, before);
  assert.equal(ofType("playback.state").length, 6);
});

test("a controller ignores incoming commands", async (t) => {
  const { store, calls, playbackStore } = harness(t, {
    playback: { mode: "remote" },
  });
  playbackStore.currentTrackId = "t1";
  store.handleMessage("playback.command", { command: "play", payload: {} });
  await flush();
  assert.equal(calls.length, 0);
});

test("sendCommand targets the controlled device by default and refuses without one", (t) => {
  const { store, ofType } = harness(t);
  store.sendCommand("play");
  assert.equal(ofType("playback.command").length, 0);
  assert.equal(store.lastError.code, "no_target");
  store.clearError();
  store.controlledDeviceId = 9;
  store.sendCommand("pause");
  store.sendCommand("seek", { position: 3 }, 4);
  const sent = ofType("playback.command").map((m) => m.payload);
  assert.deepEqual(sent, [
    { command: "pause", payload: {}, target_device_id: 9 },
    { command: "seek", payload: { position: 3 }, target_device_id: 4 },
  ]);
  store.handleMessage("playback.welcome", welcome({ device_id: 1 }));
  store.handleMessage("playback.device_state", {
    device_id: 9,
    device_name: "tv",
    state: { is_playing: true },
  });
});

test("the controlled device's state and queue are applied to the playback store", (t) => {
  t.mock.timers.enable({ apis: ["Date"], now: 42 });
  const { store, calls } = harness(t);
  store.handleMessage("playback.welcome", welcome());
  store.controlledDeviceId = 2;
  store.handleMessage("playback.device_state", {
    device_id: 2,
    device_name: "phone",
    state: { is_playing: true, position: 1 },
  });
  assert.deepEqual(calls.at(-1), [
    "applyRemoteState",
    { is_playing: true, position: 1 },
    42,
  ]);
  store.handleMessage("playback.queue_sync", {
    queue: [{ id: "q" }],
    queue_version: 2,
    context: { type: "album" },
  });
  assert.deepEqual(calls.at(-1), ["applyRemoteQueue", [{ id: "q" }], { type: "album" }]);
  assert.equal(store.otherDeviceStates[2].queueVersion, 2);
});

test("hello retries stop after an error reply and the keep-alive follows playback", (t) => {
  t.mock.timers.enable({ apis: ["setInterval", "setTimeout"] });
  const { store, ofType, keepAlive, playbackStore } = harness(t);
  store.initialize();
  assert.equal(ofType("playback.hello").length, 1);
  t.mock.timers.tick(2000);
  assert.equal(ofType("playback.hello").length, 2);
  store.handleMessage("playback.error", {
    code: "invalid_message",
    message: "Invalid hello payload",
  });
  t.mock.timers.tick(10000);
  assert.equal(ofType("playback.hello").length, 2);

  assert.equal(keepAlive()(), false);
  playbackStore.isPlaying = true;
  assert.equal(keepAlive()(), true);
  playbackStore.isPlaying = false;
  store.controlledDeviceId = 3;
  assert.equal(keepAlive()(), true);
  store.cleanup();
  assert.equal(keepAlive(), null);
});
