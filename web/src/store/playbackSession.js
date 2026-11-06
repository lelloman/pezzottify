/**
 * Playback Session Store
 *
 * Manages the WebSocket playback session protocol for multi-device state syncing.
 * Each device independently reports its playback state to the server, which
 * relays updates to other devices of the same user.
 *
 * The wire contract lives in docs/multi-device-playback.md.
 *
 * Responsibilities:
 * - Send playback.hello on WS connect, handle welcome
 * - Track connected devices and other devices' playback states
 * - Broadcast local state periodically and on events
 * - Execute commands addressed to this device
 */

import { defineStore } from "pinia";
import { ref, computed, watch } from "vue";
import * as ws from "../services/websocket";
import { getDeviceName } from "../services/oidc";
import { useRemoteStore } from "./remote";
import { useStaticsStore } from "./statics";

const BROADCAST_INTERVAL_MS = 5000;
const HELLO_RETRY_MS = 2000;
const MAX_HELLO_ATTEMPTS = 10;

// Commands the web player cannot honour (no shuffle/repeat in the web player).
const UNSUPPORTED_COMMANDS = new Set(["setShuffle", "setRepeat"]);

export const usePlaybackSessionStore = defineStore("playbackSession", () => {
  // ============================================
  // State
  // ============================================

  const myDeviceId = ref(null);
  const devices = ref([]);
  const isBroadcasting = ref(false);
  const queueVersion = ref(0);

  // Device this tab is remote-controlling (remote-mode scaffolding; the UI
  // does not enter remote mode yet). Used as the default command target.
  const controlledDeviceId = ref(null);

  // Last playback.error from the server, surfaced by the Devices UI.
  // { code, message, command, at }
  const lastError = ref(null);

  // Per-device playback states from other devices
  // { [deviceId]: { deviceName, state, receivedAt, queue, queueContext, queueVersion } }
  const otherDeviceStates = ref({});

  // ============================================
  // Computed
  // ============================================

  const otherDevicesCount = computed(() => {
    return devices.value.filter((d) => d.id !== myDeviceId.value).length;
  });

  // ============================================
  // Broadcast timer
  // ============================================

  let _broadcastInterval = null;
  let _playbackStore = null;
  let _helloRetryInterval = null;
  let _helloAttempts = 0;

  function startStateBroadcast() {
    stopStateBroadcast();
    _broadcastInterval = setInterval(() => {
      if (isBroadcasting.value && _playbackStore) {
        broadcastState();
      }
    }, BROADCAST_INTERVAL_MS);
  }

  function startHelloRetry() {
    if (_helloRetryInterval) return;
    _helloRetryInterval = setInterval(() => {
      if (!ws.wsConnected.value) return;
      if (myDeviceId.value != null) {
        stopHelloRetry();
        return;
      }
      if (_helloAttempts >= MAX_HELLO_ATTEMPTS) {
        console.warn(
          "[PlaybackSession] giving up on hello after",
          _helloAttempts,
          "attempts",
        );
        stopHelloRetry();
        return;
      }
      console.debug("[PlaybackSession] retry hello");
      sendHello();
    }, HELLO_RETRY_MS);
  }

  function stopHelloRetry() {
    if (_helloRetryInterval) {
      clearInterval(_helloRetryInterval);
      _helloRetryInterval = null;
    }
  }

  function stopStateBroadcast() {
    if (_broadcastInterval) {
      clearInterval(_broadcastInterval);
      _broadcastInterval = null;
    }
  }

  function canBroadcast() {
    return Boolean(_playbackStore?.currentTrackId) &&
      _playbackStore?.mode !== "remote";
  }

  function broadcastState() {
    if (!canBroadcast()) return;
    // A null track with a loaded track id means metadata has not resolved
    // yet. Sending it would be read by the server as the stop marker.
    if (!_playbackStore.currentTrack) {
      console.debug("[PlaybackSession] broadcastState skipped, track unresolved");
      return;
    }
    console.debug("[PlaybackSession] broadcastState", {
      trackId: _playbackStore.currentTrackId,
      isPlaying: _playbackStore.isPlaying,
      queueVersion: queueVersion.value,
    });
    const state = _playbackStore.snapshotState(queueVersion.value);
    ws.send("playback.state", state);
  }

  function broadcastQueue() {
    if (!canBroadcast()) return;
    queueVersion.value++;
    const queue = _playbackStore.snapshotQueue();
    const context = _playbackStore.snapshotQueueContext();
    console.debug("[PlaybackSession] broadcastQueue", {
      queueSize: queue.length,
      queueVersion: queueVersion.value,
      trackId: _playbackStore.currentTrackId,
    });
    ws.send("playback.queue_update", {
      queue,
      queue_version: queueVersion.value,
      context,
    });
  }

  function ensureBroadcasting() {
    if (!isBroadcasting.value) {
      isBroadcasting.value = true;
      startStateBroadcast();
    }
  }

  // Report state and queue together so peers never see one without the other.
  function announceLocalPlayback() {
    if (!canBroadcast()) return;
    ensureBroadcasting();
    broadcastState();
    broadcastQueue();
  }

  // ============================================
  // Protocol: Outgoing messages
  // ============================================

  function sendHello() {
    _helloAttempts++;
    console.debug("[PlaybackSession] sendHello", { attempt: _helloAttempts });
    ws.send("playback.hello", {
      device_name: getDeviceName(),
      device_type: "web",
    });
  }

  /**
   * Send a command to another device. Without an explicit target the
   * command goes to the device this tab is controlling.
   */
  function sendCommand(command, payload = {}, targetDeviceId = null) {
    const target = targetDeviceId ?? controlledDeviceId.value;
    if (target == null) {
      console.warn("[PlaybackSession] sendCommand without target:", command);
      lastError.value = {
        code: "no_target",
        message: "No device selected for this command",
        command,
        at: Date.now(),
      };
      return;
    }
    ws.send("playback.command", {
      command,
      payload,
      target_device_id: target,
    });
  }

  function requestQueue(targetDeviceId = null) {
    const target = targetDeviceId ?? controlledDeviceId.value;
    ws.send(
      "playback.request_queue",
      target != null ? { target_device_id: target } : null,
    );
  }

  // ============================================
  // Protocol: Incoming message handler
  // ============================================

  function handleMessage(type, payload) {
    switch (type) {
      case "playback.welcome":
        handleWelcome(payload);
        break;
      case "playback.device_state":
        handleDeviceState(payload);
        break;
      case "playback.device_queue":
        handleDeviceQueue(payload);
        break;
      case "playback.device_stopped":
        handleDeviceStopped(payload);
        break;
      case "playback.queue_sync":
        handleQueueSync(payload);
        break;
      case "playback.command":
        handleCommand(payload);
        break;
      case "playback.device_list_changed":
        handleDeviceListChanged(payload);
        break;
      case "playback.error":
        handleError(payload);
        break;
      default:
        console.warn("[PlaybackSession] Unknown message:", type);
    }
  }

  // ============================================
  // Protocol: Message handlers
  // ============================================

  function handleWelcome(payload) {
    myDeviceId.value = payload.device_id;
    devices.value = payload.devices || [];
    stopHelloRetry();
    _helloAttempts = 0;
    console.info("[PlaybackSession] welcome", {
      myDeviceId: myDeviceId.value,
      devices: devices.value.length,
      activeDevices: payload.session?.active_devices?.length || 0,
    });

    const receivedAt = Date.now();
    const states = {};
    for (const d of payload.session?.active_devices || []) {
      if (d.device_id !== payload.device_id) {
        states[d.device_id] = {
          deviceName: d.device_name,
          state: d.state,
          receivedAt,
          queue: d.queue,
          queueContext: d.context,
          queueVersion: d.queue_version,
        };
      }
    }
    otherDeviceStates.value = states;

    // On (re)connect, re-announce state and queue if we have a loaded track
    announceLocalPlayback();
  }

  function handleDeviceState(payload) {
    if (payload.device_id === myDeviceId.value) return;

    console.debug("[PlaybackSession] device_state", {
      deviceId: payload.device_id,
      isPlaying: payload.state?.is_playing,
      trackId: payload.state?.current_track?.id,
      queuePosition: payload.state?.queue_position,
      queueVersion: payload.state?.queue_version,
      timestamp: payload.state?.timestamp,
    });
    const existing = otherDeviceStates.value[payload.device_id] || {};
    otherDeviceStates.value = {
      ...otherDeviceStates.value,
      [payload.device_id]: {
        ...existing,
        deviceName: payload.device_name,
        state: payload.state,
        receivedAt: Date.now(),
      },
    };

    if (payload.device_id === controlledDeviceId.value) {
      _playbackStore?.applyRemoteState?.(payload.state, Date.now());
    }
  }

  function mergeQueue(deviceId, payload) {
    const existing = otherDeviceStates.value[deviceId] || {};
    otherDeviceStates.value = {
      ...otherDeviceStates.value,
      [deviceId]: {
        ...existing,
        queue: payload.queue,
        queueContext: payload.context,
        queueVersion: payload.queue_version,
      },
    };
    if (deviceId === controlledDeviceId.value) {
      _playbackStore?.applyRemoteQueue?.(payload.queue || [], payload.context);
    }
  }

  function handleDeviceQueue(payload) {
    if (payload.device_id === myDeviceId.value) return;

    console.debug("[PlaybackSession] device_queue", {
      deviceId: payload.device_id,
      queueSize: payload.queue?.length || 0,
      queueVersion: payload.queue_version,
    });
    mergeQueue(payload.device_id, payload);
  }

  function handleDeviceStopped(payload) {
    const newStates = { ...otherDeviceStates.value };
    delete newStates[payload.device_id];
    otherDeviceStates.value = newStates;
  }

  function handleQueueSync(payload) {
    const targetDeviceId = payload.device_id ?? controlledDeviceId.value;
    if (targetDeviceId == null) {
      console.log(
        "[PlaybackSession] Queue sync received without device id:",
        payload,
      );
      return;
    }
    if (targetDeviceId === myDeviceId.value) return;

    console.debug("[PlaybackSession] queue_sync", {
      deviceId: targetDeviceId,
      queueSize: payload.queue?.length || 0,
      queueVersion: payload.queue_version,
    });
    mergeQueue(targetDeviceId, payload);
  }

  function handleError(payload) {
    console.error("[PlaybackSession] Server error:", payload);
    lastError.value = {
      code: payload?.code || "unknown",
      message: payload?.message || "Playback error",
      command: payload?.context?.command ?? null,
      at: Date.now(),
    };
    // An error before any welcome means our hello is being rejected;
    // retrying the same payload will not help.
    if (myDeviceId.value == null && _helloRetryInterval) {
      console.warn("[PlaybackSession] hello rejected, stopping retries");
      stopHelloRetry();
    }
  }

  function clearError() {
    lastError.value = null;
  }

  // ============================================
  // Commands addressed to this device
  // ============================================

  async function resolvePlaylist(playlistId) {
    const playlist = await useRemoteStore().fetchPlaylistData(playlistId);
    if (!playlist || !Array.isArray(playlist.tracks)) {
      console.warn("[PlaybackSession] playlist not found:", playlistId);
      return null;
    }
    return playlist;
  }

  async function resolveAlbum(albumId) {
    const album = await Promise.resolve(
      useStaticsStore().waitAlbumData(albumId),
    );
    if (!album) {
      console.warn("[PlaybackSession] album not found:", albumId);
      return null;
    }
    return album;
  }

  function albumTrackIds(album) {
    return (album.discs || []).flatMap((disc) => disc.tracks || []);
  }

  function findAlbumPosition(album, trackId) {
    const discs = album.discs || [];
    for (let discIndex = 0; discIndex < discs.length; discIndex++) {
      const trackIndex = (discs[discIndex].tracks || []).indexOf(trackId);
      if (trackIndex >= 0) return { discIndex, trackIndex };
    }
    return { discIndex: 0, trackIndex: 0 };
  }

  async function executeCommand(command, cmdPayload) {
    const store = _playbackStore;
    switch (command) {
      case "play":
        store.play();
        break;
      case "pause":
        store.pause();
        break;
      case "next":
        store.skipNextTrack();
        break;
      case "prev":
        store.skipPreviousTrack();
        break;
      case "seek":
        if (cmdPayload?.position != null) {
          const durMs = store.currentTrack?.duration || 0;
          if (durMs > 0) {
            store.seekToPercentage(cmdPayload.position / (durMs / 1000));
          }
        }
        break;
      case "setVolume":
        if (cmdPayload?.volume != null) {
          store.setVolume(cmdPayload.volume);
        }
        break;
      case "setMuted":
        if (cmdPayload?.muted != null) {
          store.setMuted(cmdPayload.muted);
        }
        break;
      case "loadAlbum":
        if (cmdPayload?.albumId) {
          let discIndex = 0;
          let trackIndex = 0;
          if (cmdPayload.startTrackId) {
            const album = await resolveAlbum(cmdPayload.albumId);
            if (album) {
              ({ discIndex, trackIndex } = findAlbumPosition(
                album,
                cmdPayload.startTrackId,
              ));
            }
          }
          await store.setAlbumId(cmdPayload.albumId, discIndex, trackIndex);
        }
        break;
      case "loadPlaylist":
        if (cmdPayload?.playlistId) {
          const playlist = await resolvePlaylist(cmdPayload.playlistId);
          if (playlist) {
            const startIndex = cmdPayload.startTrackId
              ? Math.max(0, playlist.tracks.indexOf(cmdPayload.startTrackId))
              : 0;
            await store.setUserPlaylist(playlist, startIndex);
          }
        }
        break;
      case "loadSingleTrack":
        if (cmdPayload?.trackId) {
          store.setPlaylistFromTrackIds([cmdPayload.trackId], 0, true);
        }
        break;
      case "loadTrackIds":
        if (cmdPayload?.trackIds?.length) {
          store.setPlaylistFromTrackIds(
            cmdPayload.trackIds,
            0,
            true,
            cmdPayload.context,
          );
        }
        break;
      case "addAlbumToQueue":
        if (cmdPayload?.albumId) {
          const album = await resolveAlbum(cmdPayload.albumId);
          const trackIds = album ? albumTrackIds(album) : [];
          if (trackIds.length) store.addTracksToPlaylist(trackIds);
        }
        break;
      case "addPlaylistToQueue":
        if (cmdPayload?.playlistId) {
          const playlist = await resolvePlaylist(cmdPayload.playlistId);
          if (playlist?.tracks?.length) {
            store.addTracksToPlaylist(playlist.tracks);
          }
        }
        break;
      case "addTracksToQueue":
        if (cmdPayload?.trackIds?.length) {
          store.addTracksToPlaylist(cmdPayload.trackIds);
        }
        break;
      case "skipToTrack":
        if (cmdPayload?.index != null) {
          store.loadTrackIndex(cmdPayload.index);
        }
        break;
      case "removeTrack":
        if (cmdPayload?.index != null) {
          store.removeTrackFromPlaylist(cmdPayload.index);
        } else if (cmdPayload?.trackId) {
          const index =
            store.currentPlaylist?.tracksIds?.indexOf(cmdPayload.trackId) ?? -1;
          if (index >= 0) store.removeTrackFromPlaylist(index);
        } else {
          console.log("[PlaybackSession] removeTrack command missing index");
        }
        break;
      case "moveTrack":
        if (cmdPayload?.fromIndex != null && cmdPayload?.toIndex != null) {
          store.moveTrack(cmdPayload.fromIndex, cmdPayload.toIndex);
        }
        break;
      case "setGravity":
        if (cmdPayload?.gravity) {
          store.applyGravity(cmdPayload.gravity);
        }
        break;
      default:
        if (UNSUPPORTED_COMMANDS.has(command)) {
          console.info(
            "[PlaybackSession] command not supported by the web player:",
            command,
          );
        } else {
          console.warn("[PlaybackSession] Unknown command:", command);
        }
    }
  }

  async function handleCommand(payload) {
    if (!_playbackStore) return;
    if (_playbackStore.mode === "remote") {
      // A controller reports no state; executing here would play unheard audio.
      console.info("[PlaybackSession] ignoring command while controlling", payload);
      return;
    }

    const { command, payload: cmdPayload } = payload;
    console.info("[PlaybackSession] command", { command, payload: cmdPayload });
    try {
      await executeCommand(command, cmdPayload);
    } catch (e) {
      console.error("[PlaybackSession] command failed:", command, e);
    }
    // Let the sender see the outcome right away, even if nothing changed.
    if (canBroadcast()) {
      ensureBroadcasting();
      broadcastState();
    }
  }

  function handleDeviceListChanged(payload) {
    devices.value = payload.devices || [];
  }

  // ============================================
  // Public API: Notifications from playback store
  // ============================================

  function notifyPlaybackStarted() {
    announceLocalPlayback();
  }

  function notifyStateChanged() {
    if (isBroadcasting.value) {
      broadcastState();
    }
  }

  function notifyQueueChanged() {
    ensureBroadcasting();
    broadcastQueue();
  }

  function notifyStopped() {
    stopStateBroadcast();
    isBroadcasting.value = false;

    // Send the stop marker so the server removes us immediately
    ws.send("playback.state", {
      current_track: null,
      queue_position: 0,
      queue_version: 0,
      position: 0,
      is_playing: false,
      volume: 1,
      muted: false,
      shuffle: false,
      repeat: "off",
      timestamp: Date.now(),
    });
  }

  // ============================================
  // Lifecycle
  // ============================================

  function setPlaybackStore(store) {
    _playbackStore = store;
  }

  // While this tab plays or reports, the socket must stay alive in the
  // background so the device stays reachable.
  function needsKeepAlive() {
    return (
      isBroadcasting.value ||
      controlledDeviceId.value != null ||
      Boolean(_playbackStore?.isPlaying)
    );
  }

  let _connectWatcher = null;

  function initialize() {
    ws.setKeepAliveProvider(needsKeepAlive);

    // Send hello now if connected, and on every reconnect
    _helloAttempts = 0;
    if (ws.wsConnected.value) {
      sendHello();
    }
    startHelloRetry();

    if (!_connectWatcher) {
      _connectWatcher = watch(ws.wsConnected, (connected) => {
        if (connected) {
          // Forget the previous connection's view of the world; the device
          // id is stable but the welcome will tell us again.
          myDeviceId.value = null;
          devices.value = [];
          otherDeviceStates.value = {};
          _helloAttempts = 0;
          sendHello();
          startHelloRetry();
        }
      });
    }
  }

  function cleanup() {
    stopStateBroadcast();
    stopHelloRetry();
    ws.setKeepAliveProvider(null);
    if (_connectWatcher) {
      _connectWatcher();
      _connectWatcher = null;
    }
    isBroadcasting.value = false;
    myDeviceId.value = null;
    devices.value = [];
    otherDeviceStates.value = {};
    controlledDeviceId.value = null;
    lastError.value = null;
    queueVersion.value = 0;
    _helloAttempts = 0;
    _playbackStore = null;
  }

  return {
    // State
    myDeviceId,
    devices,
    isBroadcasting,
    otherDeviceStates,
    controlledDeviceId,
    lastError,

    // Computed
    otherDevicesCount,

    // Protocol
    handleMessage,
    sendHello,
    sendCommand,
    requestQueue,
    clearError,

    // Notifications from playback store
    notifyPlaybackStarted,
    notifyStateChanged,
    notifyQueueChanged,
    notifyStopped,

    // Lifecycle
    setPlaybackStore,
    initialize,
    cleanup,
  };
});
