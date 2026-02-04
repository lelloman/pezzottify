/**
 * Playback Session Store
 *
 * Manages the WebSocket playback session protocol for multi-device control.
 * Follows the same pattern as sync.js - a Pinia store that registers a WS handler
 * via registerHandler("playback", ...).
 *
 * Responsibilities:
 * - Send playback.hello on WS connect, handle welcome
 * - Track connected devices
 * - Register as audio device when local playback starts
 * - Broadcast state periodically (5s) and on events when audio device
 * - Receive and dispatch remote commands when audio device
 * - Receive and apply remote state when in remote mode
 * - Handle device transfer protocol
 */

import { defineStore } from "pinia";
import { ref, computed, watch } from "vue";
import * as ws from "../services/websocket";

const BROADCAST_INTERVAL_MS = 5000;
const REMOTE_CONTROL_PREF_KEY = "remoteControlPreference";

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

  const role = ref("idle"); // 'idle' | 'audioDevice' | 'remote'
  const myDeviceId = ref(null);
  const devices = ref([]);
  const audioDeviceId = ref(null);
  const remoteState = ref(null);
  const remoteQueue = ref(null);
  const remoteQueueVersion = ref(0);
  const sessionExists = ref(false);
  const pendingTransferId = ref(null);

  // ============================================
  // Computed
  // ============================================

  const audioDeviceName = computed(() => {
    if (!audioDeviceId.value) return null;
    const device = devices.value.find((d) => d.id === audioDeviceId.value);
    return device?.name || null;
  });

  const isRemote = computed(() => role.value === "remote");
  const isAudioDevice = computed(() => role.value === "audioDevice");

  const otherDevicesCount = computed(() => {
    return devices.value.filter((d) => d.id !== myDeviceId.value).length;
  });

  // ============================================
  // Broadcast timer
  // ============================================

  let _broadcastInterval = null;
  let _playbackStore = null;

  function startStateBroadcast() {
    stopStateBroadcast();
    _broadcastInterval = setInterval(() => {
      if (role.value === "audioDevice" && _playbackStore) {
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
    if (role.value !== "audioDevice" || !_playbackStore) return;
    const state = _playbackStore.snapshotState(remoteQueueVersion.value);
    ws.send("playback.state", state);
  }

  function broadcastQueue() {
    if (role.value !== "audioDevice" || !_playbackStore) return;
    remoteQueueVersion.value++;
    const queue = _playbackStore.snapshotQueue();
    ws.send("playback.queue_update", {
      queue,
      queue_version: remoteQueueVersion.value,
    });
  }

  // ============================================
  // Remote control preference
  // ============================================

  function getRemoteControlPreference() {
    try {
      return localStorage.getItem(REMOTE_CONTROL_PREF_KEY) || "ask";
    } catch {
      return "ask";
    }
  }

  function setRemoteControlPreference(value) {
    try {
      localStorage.setItem(REMOTE_CONTROL_PREF_KEY, value);
    } catch {
      // ignore
    }
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

  function sendRegisterAudioDevice() {
    ws.send("playback.register_audio_device", {});
  }

  function sendUnregisterAudioDevice() {
    ws.send("playback.unregister_audio_device", {});
  }

  function sendCommand(command, payload = {}) {
    ws.send("playback.command", { command, payload });
  }

  // ============================================
  // Protocol: Incoming message handler
  // ============================================

  function handleMessage(type, payload) {
    switch (type) {
      case "playback.welcome":
        handleWelcome(payload);
        break;
      case "playback.register_ack":
        handleRegisterAck(payload);
        break;
      case "playback.state":
        handleRemoteState(payload);
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
      case "playback.session_ended":
        handleSessionEnded(payload);
        break;
      case "playback.prepare_transfer":
        handlePrepareTransfer(payload);
        break;
      case "playback.become_audio_device":
        handleBecomeAudioDevice(payload);
        break;
      case "playback.transfer_complete":
        handleTransferComplete(payload);
        break;
      case "playback.transfer_aborted":
        handleTransferAborted(payload);
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
    sessionExists.value = payload.session?.exists || false;

    if (payload.session?.audio_device_id != null) {
      audioDeviceId.value = payload.session.audio_device_id;
    }

    // If a session exists and we're not the audio device, check preference
    if (
      sessionExists.value &&
      audioDeviceId.value != null &&
      audioDeviceId.value !== myDeviceId.value
    ) {
      const pref = getRemoteControlPreference();
      if (pref === "always") {
        enterRemoteMode(payload.session.state, payload.session.queue);
      }
      // "ask" is handled by UI (shows banner)
      // "never" does nothing
    }

    console.log(
      "[PlaybackSession] Welcome, device:",
      myDeviceId.value,
      "session exists:",
      sessionExists.value,
    );
  }

  function handleRegisterAck(payload) {
    if (payload.success) {
      role.value = "audioDevice";
      sessionExists.value = true;
      audioDeviceId.value = myDeviceId.value;
      startStateBroadcast();
      broadcastState();
      broadcastQueue();
      console.log("[PlaybackSession] Registered as audio device");
    } else {
      console.warn("[PlaybackSession] Registration failed:", payload.error);
    }
  }

  function handleRemoteState(state) {
    remoteState.value = state;
    if (role.value === "remote" && _playbackStore) {
      _playbackStore.applyRemoteState(state);
    }
  }

  function handleQueueSync(payload) {
    remoteQueue.value = payload.queue;
    remoteQueueVersion.value = payload.queue_version;
    if (role.value === "remote" && _playbackStore) {
      _playbackStore.applyRemoteQueue(payload.queue);
    }
  }

  function handleCommand(payload) {
    if (role.value !== "audioDevice" || !_playbackStore) return;

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
          const dur = _playbackStore.currentTrack?.duration || 0;
          if (dur > 0) {
            _playbackStore.seekToPercentage(cmdPayload.position / dur);
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
      default:
        console.warn("[PlaybackSession] Unknown command:", command);
    }
  }

  function handleDeviceListChanged(payload) {
    devices.value = payload.devices || [];

    // Update audio device ID from the list
    const audio = devices.value.find((d) => d.is_audio_device);
    audioDeviceId.value = audio?.id || null;

    // Check if the session still exists (any audio device)
    sessionExists.value = audioDeviceId.value != null;
  }

  function handleSessionEnded(payload) {
    console.log("[PlaybackSession] Session ended:", payload.reason);
    sessionExists.value = false;
    audioDeviceId.value = null;

    if (role.value === "remote") {
      exitRemoteMode();
    } else if (role.value === "audioDevice") {
      stopStateBroadcast();
      role.value = "idle";
    }
  }

  // ============================================
  // Transfer protocol handlers
  // ============================================

  function handlePrepareTransfer(payload) {
    // We are the current audio device, someone wants to take over
    if (role.value !== "audioDevice" || !_playbackStore) return;

    _playbackStore.pause();
    const state = _playbackStore.snapshotState(remoteQueueVersion.value);
    const queue = _playbackStore.snapshotQueue();

    ws.send("playback.transfer_ready", {
      transfer_id: payload.transfer_id,
      state,
      queue,
    });
  }

  function handleBecomeAudioDevice(payload) {
    // We are the new audio device after transfer
    pendingTransferId.value = null;
    role.value = "audioDevice";
    sessionExists.value = true;
    audioDeviceId.value = myDeviceId.value;

    if (_playbackStore) {
      _playbackStore.exitRemoteMode();
      _playbackStore.assumeFromTransfer(payload.state, payload.queue);
    }

    ws.send("playback.transfer_complete", {
      transfer_id: payload.transfer_id,
    });

    startStateBroadcast();
  }

  function handleTransferComplete() {
    // We were the old audio device, transfer is done
    stopStateBroadcast();
    role.value = "idle";

    if (_playbackStore) {
      _playbackStore.stop();
    }
  }

  function handleTransferAborted(payload) {
    pendingTransferId.value = null;
    console.warn("[PlaybackSession] Transfer aborted:", payload.reason);

    // If we were the audio device, resume
    if (role.value === "audioDevice" && _playbackStore) {
      _playbackStore.play();
    }
  }

  // ============================================
  // Public API: Notifications from playback store
  // ============================================

  function notifyPlaybackStarted() {
    if (role.value === "idle" && !sessionExists.value) {
      sendRegisterAudioDevice();
    }
  }

  function notifyStateChanged() {
    if (role.value === "audioDevice") {
      broadcastState();
    }
  }

  function notifyQueueChanged() {
    if (role.value === "audioDevice") {
      broadcastQueue();
    }
  }

  // ============================================
  // Public API: Remote mode control
  // ============================================

  function enterRemoteMode(initialState = null, initialQueue = null) {
    if (!_playbackStore) return;

    _playbackStore.enterRemoteMode();
    role.value = "remote";

    if (initialState) {
      _playbackStore.applyRemoteState(initialState);
    } else if (remoteState.value) {
      _playbackStore.applyRemoteState(remoteState.value);
    }

    if (initialQueue) {
      _playbackStore.applyRemoteQueue(initialQueue);
    } else if (remoteQueue.value) {
      _playbackStore.applyRemoteQueue(remoteQueue.value);
    }
  }

  function exitRemoteMode() {
    if (!_playbackStore) return;

    _playbackStore.exitRemoteMode();
    role.value = "idle";
    remoteState.value = null;
    remoteQueue.value = null;
  }

  function requestTakeover() {
    const transferId = crypto.randomUUID();
    pendingTransferId.value = transferId;
    sendCommand("becomeAudioDevice", { transfer_id: transferId });
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
          role.value = "idle";
          myDeviceId.value = null;
          devices.value = [];
          audioDeviceId.value = null;
          sessionExists.value = false;
          sendHello();

          // Re-register as audio device if we were playing locally
          if (_playbackStore?.isPlaying && _playbackStore?.mode === "local") {
            sendRegisterAudioDevice();
          }
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
    if (role.value === "audioDevice") {
      sendUnregisterAudioDevice();
    }
    role.value = "idle";
    myDeviceId.value = null;
    devices.value = [];
    audioDeviceId.value = null;
    remoteState.value = null;
    remoteQueue.value = null;
    remoteQueueVersion.value = 0;
    sessionExists.value = false;
    pendingTransferId.value = null;
    _playbackStore = null;
  }

  return {
    // State
    role,
    myDeviceId,
    devices,
    audioDeviceId,
    remoteState,
    remoteQueue,
    remoteQueueVersion,
    sessionExists,
    pendingTransferId,

    // Computed
    audioDeviceName,
    isRemote,
    isAudioDevice,
    otherDevicesCount,

    // Protocol
    handleMessage,
    sendHello,
    sendCommand,

    // Notifications from playback store
    notifyPlaybackStarted,
    notifyStateChanged,
    notifyQueueChanged,

    // Remote mode
    enterRemoteMode,
    exitRemoteMode,
    requestTakeover,

    // Preference
    getRemoteControlPreference,
    setRemoteControlPreference,

    // Lifecycle
    setPlaybackStore,
    initialize,
    cleanup,
  };
});
