/**
 * Playback Session Store
 *
 * Manages the WebSocket playback session protocol for multi-device state syncing.
 * Each device independently reports its playback state to the server, which
 * relays updates to other devices of the same user.
 *
 * Responsibilities:
 * - Send playback.hello on WS connect, handle welcome
 * - Track connected devices and other devices' playback states
 * - Broadcast local state periodically and on events
 * - Display other devices' playback status
 */

import { defineStore } from "pinia";
import { ref, computed, watch } from "vue";
import * as ws from "../services/websocket";

const BROADCAST_INTERVAL_MS = 5000;

/**
 * Detect a human-readable device name from the browser.
 */
function detectDeviceName() {
  let browser = "Browser";
  let os = "Unknown";

  const ua = navigator.userAgent;

  if (ua.includes("Firefox/")) browser = "Firefox";
  else if (ua.includes("Edg/")) browser = "Edge";
  else if (ua.includes("Chrome/") && !ua.includes("Edg/")) browser = "Chrome";
  else if (ua.includes("Safari/") && !ua.includes("Chrome/")) browser = "Safari";

  if (navigator.userAgentData?.platform) {
    os = navigator.userAgentData.platform;
  } else if (ua.includes("Windows")) {
    os = "Windows";
  } else if (ua.includes("Mac OS")) {
    os = "macOS";
  } else if (ua.includes("Linux")) {
    os = "Linux";
  } else if (ua.includes("Android")) {
    os = "Android";
  } else if (ua.includes("iPhone") || ua.includes("iPad")) {
    os = "iOS";
  }

  return `${browser} on ${os}`;
}

export const usePlaybackSessionStore = defineStore("playbackSession", () => {
  // ============================================
  // State
  // ============================================

  const myDeviceId = ref(null);
  const devices = ref([]);
  const isBroadcasting = ref(false);
  const queueVersion = ref(0);

  // Per-device playback states from other devices
  // { [deviceId]: { deviceName, state, queue, queueVersion } }
  const otherDeviceStates = ref({});

  // ============================================
  // Computed
  // ============================================

  const anyOtherDevicePlaying = computed(
    () => Object.keys(otherDeviceStates.value).length > 0,
  );

  const otherDevicesCount = computed(() => {
    return devices.value.filter((d) => d.id !== myDeviceId.value).length;
  });

  const otherPlayingDeviceNames = computed(() => {
    return Object.values(otherDeviceStates.value)
      .map((d) => d.deviceName)
      .filter(Boolean);
  });

  // ============================================
  // Broadcast timer
  // ============================================

  let _broadcastInterval = null;
  let _playbackStore = null;

  function startStateBroadcast() {
    stopStateBroadcast();
    _broadcastInterval = setInterval(() => {
      if (isBroadcasting.value && _playbackStore) {
        broadcastState();
      }
    }, BROADCAST_INTERVAL_MS);
  }

  function stopStateBroadcast() {
    if (_broadcastInterval) {
      clearInterval(_broadcastInterval);
      _broadcastInterval = null;
    }
  }

  function broadcastState() {
    if (!_playbackStore?.currentTrackId || _playbackStore?.mode === "remote")
      return;
    const state = _playbackStore.snapshotState(queueVersion.value);
    ws.send("playback.state", state);
  }

  function broadcastQueue() {
    if (!_playbackStore?.currentTrackId || _playbackStore?.mode === "remote")
      return;
    queueVersion.value++;
    const queue = _playbackStore.snapshotQueue();
    ws.send("playback.queue_update", {
      queue,
      queue_version: queueVersion.value,
    });
  }

  // ============================================
  // Protocol: Outgoing messages
  // ============================================

  function sendHello() {
    ws.send("playback.hello", {
      device_name: detectDeviceName(),
      device_type: "web",
    });
  }

  function sendCommand(command, payload = {}, targetDeviceId = null) {
    const msg = { command, payload };
    if (targetDeviceId != null) {
      msg.target_device_id = targetDeviceId;
    }
    ws.send("playback.command", msg);
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
        console.error("[PlaybackSession] Server error:", payload);
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

    const states = {};
    for (const d of payload.session?.active_devices || []) {
      if (d.device_id !== payload.device_id) {
        states[d.device_id] = {
          deviceName: d.device_name,
          state: d.state,
          queue: d.queue,
          queueVersion: d.queue_version,
        };
      }
    }
    otherDeviceStates.value = states;

    // On reconnect, re-announce if we have a loaded track
    if (_playbackStore?.currentTrackId && _playbackStore?.mode !== "remote") {
      if (!isBroadcasting.value) {
        isBroadcasting.value = true;
        startStateBroadcast();
      }
      broadcastState();
      broadcastQueue();
    }

    console.log(
      "[PlaybackSession] Welcome, device:",
      myDeviceId.value,
      "other active devices:",
      Object.keys(states).length,
    );
  }

  function handleDeviceState(payload) {
    if (payload.device_id === myDeviceId.value) return;

    const existing = otherDeviceStates.value[payload.device_id] || {};
    otherDeviceStates.value = {
      ...otherDeviceStates.value,
      [payload.device_id]: {
        ...existing,
        deviceName: payload.device_name,
        state: payload.state,
      },
    };
  }

  function handleDeviceQueue(payload) {
    if (payload.device_id === myDeviceId.value) return;

    const existing = otherDeviceStates.value[payload.device_id] || {};
    otherDeviceStates.value = {
      ...otherDeviceStates.value,
      [payload.device_id]: {
        ...existing,
        queue: payload.queue,
        queueVersion: payload.queue_version,
      },
    };
  }

  function handleDeviceStopped(payload) {
    const newStates = { ...otherDeviceStates.value };
    delete newStates[payload.device_id];
    otherDeviceStates.value = newStates;
  }

  function handleQueueSync(payload) {
    const targetDeviceId = payload.device_id;
    if (!targetDeviceId) {
      console.log("[PlaybackSession] Queue sync received without device id:", payload);
      return;
    }

    const existing = otherDeviceStates.value[targetDeviceId] || {};
    otherDeviceStates.value = {
      ...otherDeviceStates.value,
      [targetDeviceId]: {
        ...existing,
        queue: payload.queue,
        queueVersion: payload.queue_version,
      },
    };
  }

  function handleCommand(payload) {
    if (!_playbackStore) return;

    const { command, payload: cmdPayload } = payload;
    switch (command) {
      case "play":
        _playbackStore.play();
        break;
      case "pause":
        _playbackStore.pause();
        break;
      case "next":
        _playbackStore.skipNextTrack();
        break;
      case "prev":
        _playbackStore.skipPreviousTrack();
        break;
      case "seek":
        if (cmdPayload?.position != null) {
          const durMs = _playbackStore.currentTrack?.duration || 0;
          if (durMs > 0) {
            _playbackStore.seekToPercentage(cmdPayload.position / (durMs / 1000));
          }
        }
        break;
      case "setVolume":
        if (cmdPayload?.volume != null) {
          _playbackStore.setVolume(cmdPayload.volume);
        }
        break;
      case "setMuted":
        if (cmdPayload?.muted != null) {
          _playbackStore.setMuted(cmdPayload.muted);
        }
        break;
      case "loadAlbum":
        if (cmdPayload?.albumId) {
          _playbackStore.setAlbumId(cmdPayload.albumId, 0, 0);
        }
        break;
      case "loadPlaylist":
        if (cmdPayload?.playlistId) {
          // Load playlist by ID - need to fetch playlist first
          console.log(
            "[PlaybackSession] loadPlaylist command:",
            cmdPayload.playlistId,
          );
        }
        break;
      case "loadSingleTrack":
        if (cmdPayload?.trackId) {
          _playbackStore.setPlaylistFromTrackIds([cmdPayload.trackId], 0, true);
        }
        break;
      case "addAlbumToQueue":
        // Album queue addition requires fetching album tracks first
        console.log(
          "[PlaybackSession] addAlbumToQueue command:",
          cmdPayload?.albumId,
        );
        break;
      case "addPlaylistToQueue":
        console.log(
          "[PlaybackSession] addPlaylistToQueue command:",
          cmdPayload?.playlistId,
        );
        break;
      case "addTracksToQueue":
        if (cmdPayload?.trackIds) {
          _playbackStore.addTracksToPlaylist(cmdPayload.trackIds);
        }
        break;
      case "skipToTrack":
        if (cmdPayload?.index != null) {
          _playbackStore.loadTrackIndex(cmdPayload.index);
        }
        break;
      case "setShuffle":
        // Shuffle not yet implemented in web player
        console.log(
          "[PlaybackSession] setShuffle command:",
          cmdPayload?.enabled,
        );
        break;
      case "setRepeat":
        // Repeat not yet implemented in web player
        console.log("[PlaybackSession] setRepeat command:", cmdPayload?.mode);
        break;
      case "removeTrack":
        if (cmdPayload?.index != null) {
          _playbackStore.removeTrackFromPlaylist(cmdPayload.index);
        } else {
          console.log("[PlaybackSession] removeTrack command missing index");
        }
        break;
      case "moveTrack":
        if (cmdPayload?.fromIndex != null && cmdPayload?.toIndex != null) {
          _playbackStore.moveTrack(cmdPayload.fromIndex, cmdPayload.toIndex);
        }
        break;
      default:
        console.warn("[PlaybackSession] Unknown command:", command);
    }
  }

  function handleDeviceListChanged(payload) {
    devices.value = payload.devices || [];
  }

  // ============================================
  // Public API: Notifications from playback store
  // ============================================

  function notifyPlaybackStarted() {
    if (!isBroadcasting.value) {
      isBroadcasting.value = true;
      startStateBroadcast();
    }
    broadcastState();
    broadcastQueue();
  }

  function notifyStateChanged() {
    if (isBroadcasting.value) {
      broadcastState();
    }
  }

  function notifyQueueChanged() {
    if (!isBroadcasting.value) {
      isBroadcasting.value = true;
      startStateBroadcast();
    }
    broadcastQueue();
  }

  function notifyStopped() {
    stopStateBroadcast();
    isBroadcasting.value = false;

    // Send a final stopped state so server removes us immediately
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

  let _connectWatcher = null;

  function initialize() {
    // Send hello now if connected, and on every reconnect
    if (ws.wsConnected.value) {
      sendHello();
    }

    if (!_connectWatcher) {
      _connectWatcher = watch(ws.wsConnected, (connected) => {
        if (connected) {
          // Reset state on reconnect - server assigns new device ID
          myDeviceId.value = null;
          devices.value = [];
          otherDeviceStates.value = {};
          sendHello();
        }
      });
    }
  }

  function cleanup() {
    stopStateBroadcast();
    if (_connectWatcher) {
      _connectWatcher();
      _connectWatcher = null;
    }
    isBroadcasting.value = false;
    myDeviceId.value = null;
    devices.value = [];
    otherDeviceStates.value = {};
    queueVersion.value = 0;
    _playbackStore = null;
  }

  return {
    // State
    myDeviceId,
    devices,
    isBroadcasting,
    otherDeviceStates,

    // Computed
    anyOtherDevicePlaying,
    otherDevicesCount,
    otherPlayingDeviceNames,

    // Protocol
    handleMessage,
    sendHello,
    sendCommand,

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
