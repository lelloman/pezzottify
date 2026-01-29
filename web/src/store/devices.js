/**
 * Devices Store - Device management for remote playback
 *
 * Handles device discovery, WebSocket message handling, audio device registration,
 * state broadcasting, remote command sending, and transfer protocol.
 */

import { defineStore } from "pinia";
import { ref, computed, watch } from "vue";
import * as ws from "@/services/websocket";

export const useDevicesStore = defineStore("devices", () => {
  // ============================================
  // State
  // ============================================

  // Connection state
  const deviceId = ref(null);
  const devices = ref([]);
  const initialized = ref(false);

  // Session state
  const sessionExists = ref(false);
  const reclaimable = ref(false);

  // Remote state (when controlling remote device)
  const remoteState = ref(null);
  const remoteQueue = ref([]);
  const remoteQueueVersion = ref(0);

  // Transfer state
  const pendingTransfer = ref(null);

  // Broadcast state (when this device is audio device)
  let broadcastInterval = null;
  const BROADCAST_INTERVAL_MS = 5000;
  const queueVersion = ref(0);

  // Callbacks for playback store
  let playbackCallbacks = null;

  // ============================================
  // Computed
  // ============================================

  const audioDevice = computed(() =>
    devices.value.find((d) => d.is_audio_device)
  );

  const isAudioDevice = computed(() => {
    const ad = audioDevice.value;
    return ad && ad.id === deviceId.value;
  });

  const otherDevices = computed(() =>
    devices.value.filter((d) => d.id !== deviceId.value)
  );

  const thisDevice = computed(() =>
    devices.value.find((d) => d.id === deviceId.value)
  );

  // ============================================
  // Initialization
  // ============================================

  function setPlaybackCallbacks(callbacks) {
    playbackCallbacks = callbacks;
  }

  function initialize() {
    if (initialized.value) return;

    ws.registerHandler("playback", handlePlaybackMessage);
    sendHello();
    initialized.value = true;
  }

  function sendHello() {
    const deviceName = generateDeviceName();
    const deviceType = detectDeviceType();
    ws.send("playback.hello", {
      device_name: deviceName,
      device_type: deviceType,
    });
  }

  function generateDeviceName() {
    const browser = detectBrowser();
    const os = detectOS();
    return `${browser} on ${os}`;
  }

  function detectBrowser() {
    const ua = navigator.userAgent;
    if (ua.includes("Firefox")) return "Firefox";
    if (ua.includes("Edg/")) return "Edge";
    if (ua.includes("Chrome")) return "Chrome";
    if (ua.includes("Safari")) return "Safari";
    if (ua.includes("Opera") || ua.includes("OPR")) return "Opera";
    return "Browser";
  }

  function detectOS() {
    const ua = navigator.userAgent;
    if (ua.includes("Windows")) return "Windows";
    if (ua.includes("Mac OS")) return "macOS";
    if (ua.includes("Linux")) return "Linux";
    if (ua.includes("Android")) return "Android";
    if (ua.includes("iPhone") || ua.includes("iPad")) return "iOS";
    return "Unknown";
  }

  function detectDeviceType() {
    const ua = navigator.userAgent;
    if (ua.includes("Android")) return "android";
    if (ua.includes("iPhone") || ua.includes("iPad")) return "ios";
    return "web";
  }

  // ============================================
  // Message handling
  // ============================================

  function handlePlaybackMessage(type, payload) {
    console.log("[Devices] Received:", type, payload);

    switch (type) {
      case "playback.welcome":
        handleWelcome(payload);
        break;
      case "playback.state":
        handleRemoteState(payload);
        break;
      case "playback.queue_sync":
      case "playback.queue_update":
        handleQueueSync(payload);
        break;
      case "playback.session_ended":
        handleSessionEnded(payload);
        break;
      case "playback.device_list_changed":
        handleDeviceListChanged(payload);
        break;
      case "playback.command":
        handleCommand(payload);
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
    }
  }

  function handleWelcome(payload) {
    deviceId.value = payload.device_id;
    devices.value = payload.devices;
    sessionExists.value = payload.session.exists;
    reclaimable.value = payload.session.reclaimable || false;

    if (payload.session.exists && payload.session.state) {
      remoteState.value = payload.session.state;
      remoteQueue.value = payload.session.queue || [];
      remoteQueueVersion.value = payload.session.state.queue_version;
    }

    // Notify playback store of welcome
    if (playbackCallbacks?.onWelcome) {
      playbackCallbacks.onWelcome(payload);
    }

    console.log("[Devices] Welcome received, device ID:", deviceId.value);
  }

  function handleRemoteState(payload) {
    if (isAudioDevice.value) return; // Ignore if we're the audio device

    remoteState.value = payload;

    // Check queue version
    if (payload.queue_version > remoteQueueVersion.value) {
      requestQueueSync();
    }

    // Notify playback store
    if (playbackCallbacks?.onRemoteState) {
      playbackCallbacks.onRemoteState(payload);
    }
  }

  function handleQueueSync(payload) {
    remoteQueue.value = payload.queue;
    remoteQueueVersion.value = payload.queue_version;

    if (playbackCallbacks?.onQueueSync) {
      playbackCallbacks.onQueueSync(payload);
    }
  }

  function handleSessionEnded(payload) {
    console.log("[Devices] Session ended:", payload?.reason);
    sessionExists.value = false;
    remoteState.value = null;
    remoteQueue.value = [];
    stopBroadcasting();

    if (playbackCallbacks?.onSessionEnded) {
      playbackCallbacks.onSessionEnded(payload);
    }
  }

  function handleDeviceListChanged(payload) {
    devices.value = payload.devices;

    const currentAudioDevice = devices.value.find((d) => d.is_audio_device);
    if (currentAudioDevice) {
      sessionExists.value = true;
    } else {
      sessionExists.value = false;
      stopBroadcasting();
    }

    // Notify playback store
    if (playbackCallbacks?.onDeviceListChanged) {
      playbackCallbacks.onDeviceListChanged(payload);
    }

    console.log(
      "[Devices] Device list changed:",
      payload.change.type,
      payload.change.device_id
    );
  }

  function handleCommand(payload) {
    // Forward command to playback store if we're the audio device
    if (isAudioDevice.value && playbackCallbacks?.onCommand) {
      playbackCallbacks.onCommand(payload);
    }
  }

  function handlePrepareTransfer(payload) {
    pendingTransfer.value = {
      transferId: payload.transfer_id,
      type: "source",
    };

    if (playbackCallbacks?.onPrepareTransfer) {
      playbackCallbacks.onPrepareTransfer(payload);
    }

    console.log("[Devices] Preparing transfer to", payload.target_device_name);
  }

  function handleBecomeAudioDevice(payload) {
    pendingTransfer.value = null;
    sessionExists.value = true;

    // Mark ourselves as audio device locally
    devices.value = devices.value.map((d) => ({
      ...d,
      is_audio_device: d.id === deviceId.value,
    }));

    if (playbackCallbacks?.onBecomeAudioDevice) {
      playbackCallbacks.onBecomeAudioDevice(payload);
    }

    console.log("[Devices] Became audio device via transfer");
  }

  function handleTransferComplete(payload) {
    // Old audio device - transfer succeeded
    pendingTransfer.value = null;
    stopBroadcasting();

    if (playbackCallbacks?.onTransferComplete) {
      playbackCallbacks.onTransferComplete(payload);
    }

    console.log("[Devices] Transfer complete");
  }

  function handleTransferAborted(payload) {
    console.log("[Devices] Transfer aborted:", payload.reason);
    pendingTransfer.value = null;

    if (playbackCallbacks?.onTransferAborted) {
      playbackCallbacks.onTransferAborted(payload);
    }
  }

  // ============================================
  // Audio device management
  // ============================================

  function registerAsAudioDevice() {
    console.log("[Devices] Registering as audio device");
    ws.send("playback.register_audio_device", {});
    sessionExists.value = true;

    // Mark ourselves as audio device locally
    devices.value = devices.value.map((d) => ({
      ...d,
      is_audio_device: d.id === deviceId.value,
    }));

    startBroadcasting();
  }

  function unregisterAsAudioDevice() {
    ws.send("playback.unregister_audio_device", {});
    stopBroadcasting();
    console.log("[Devices] Unregistered as audio device");
  }

  function reclaimAudioDevice(state) {
    ws.send("playback.reclaim_audio_device", state);

    // Mark ourselves as audio device locally
    devices.value = devices.value.map((d) => ({
      ...d,
      is_audio_device: d.id === deviceId.value,
    }));

    startBroadcasting();
    console.log("[Devices] Reclaimed audio device status");
  }

  // ============================================
  // Broadcasting (when audio device)
  // ============================================

  function startBroadcasting() {
    if (broadcastInterval) return;

    broadcastInterval = setInterval(() => {
      if (playbackCallbacks?.getPlaybackState) {
        const state = playbackCallbacks.getPlaybackState();
        if (state.isPlaying) {
          broadcastState(state);
        }
      }
    }, BROADCAST_INTERVAL_MS);

    // Broadcast immediately
    if (playbackCallbacks?.getPlaybackState) {
      broadcastState(playbackCallbacks.getPlaybackState());
    }
  }

  function stopBroadcasting() {
    if (broadcastInterval) {
      clearInterval(broadcastInterval);
      broadcastInterval = null;
    }
  }

  function broadcastState(state) {
    if (!isAudioDevice.value) return;
    ws.send("playback.state", state);
  }

  function broadcastStateNow() {
    if (!isAudioDevice.value) return;
    if (playbackCallbacks?.getPlaybackState) {
      broadcastState(playbackCallbacks.getPlaybackState());
    }
  }

  function broadcastQueueUpdate(queue) {
    if (!isAudioDevice.value) return;
    queueVersion.value++;
    ws.send("playback.queue_update", {
      queue,
      queue_version: queueVersion.value,
    });
  }

  // ============================================
  // Remote commands
  // ============================================

  function sendCommand(command, payload = null) {
    ws.send("playback.command", { command, payload });
  }

  function requestQueueSync() {
    ws.send("playback.request_queue", {});
  }

  // ============================================
  // Transfer
  // ============================================

  function requestBecomeAudioDevice() {
    const transferId = crypto.randomUUID();
    pendingTransfer.value = { transferId, type: "requesting" };
    ws.send("playback.command", {
      command: "becomeAudioDevice",
      payload: { transfer_id: transferId },
    });
    console.log("[Devices] Requesting to become audio device");
  }

  function sendTransferReady(transferId, state, queue) {
    ws.send("playback.transfer_ready", {
      transfer_id: transferId,
      state,
      queue,
    });
  }

  function confirmTransferComplete(transferId) {
    ws.send("playback.transfer_complete", { transfer_id: transferId });
  }

  // ============================================
  // WebSocket connection watch
  // ============================================

  watch(
    () => ws.wsConnected.value,
    (connected) => {
      if (connected && !initialized.value) {
        initialize();
      } else if (!connected) {
        initialized.value = false;
        deviceId.value = null;
        devices.value = [];
        stopBroadcasting();
      }
    },
    { immediate: true }
  );

  // ============================================
  // Cleanup
  // ============================================

  function cleanup() {
    stopBroadcasting();
    ws.unregisterHandler("playback");
  }

  // ============================================
  // Exports
  // ============================================

  return {
    // State
    deviceId,
    devices,
    sessionExists,
    reclaimable,
    remoteState,
    remoteQueue,
    pendingTransfer,
    queueVersion,

    // Computed
    audioDevice,
    isAudioDevice,
    otherDevices,
    thisDevice,

    // Initialization
    setPlaybackCallbacks,

    // Audio device management
    registerAsAudioDevice,
    unregisterAsAudioDevice,
    reclaimAudioDevice,

    // Broadcasting
    broadcastStateNow,
    broadcastQueueUpdate,

    // Remote commands
    sendCommand,
    requestQueueSync,

    // Transfer
    requestBecomeAudioDevice,
    sendTransferReady,
    confirmTransferComplete,

    // Cleanup
    cleanup,
  };
});
