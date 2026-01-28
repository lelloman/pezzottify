/**
 * Remote playback store for multi-device playback sync.
 *
 * Manages remote playback state, device list, and communication with the server
 * via WebSocket for coordinating playback across multiple devices.
 */

import { defineStore } from "pinia";
import { ref, computed, watch } from "vue";
import * as ws from "@/services/websocket";
import { usePlayerStore } from "./player";
import { useStaticsStore } from "./statics";

export const useRemotePlaybackStore = defineStore("remotePlayback", () => {
  // Connection state
  const deviceId = ref(null);
  const devices = ref([]);
  const isAudioDevice = ref(false);
  const sessionExists = ref(false);
  const reclaimable = ref(false);
  const initialized = ref(false);

  // Remote state (when not audio device)
  const remoteState = ref(null);
  const remoteQueue = ref([]);
  const remoteQueueVersion = ref(0);

  // Transfer state
  const pendingTransfer = ref(null);

  // Interpolated position (updated via requestAnimationFrame)
  const interpolatedPosition = ref(0);
  let interpolationFrame = null;

  // Broadcast interval
  let broadcastInterval = null;
  const BROADCAST_INTERVAL_MS = 5000;

  // Queue version for tracking changes
  const queueVersion = ref(0);

  // Computed
  const audioDevice = computed(() =>
    devices.value.find((d) => d.is_audio_device),
  );

  const otherDevices = computed(() =>
    devices.value.filter((d) => d.id !== deviceId.value),
  );

  const isRemoteMode = computed(
    () => sessionExists.value && !isAudioDevice.value,
  );

  // Initialize - called after WebSocket connects
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

  // Message handler
  function handlePlaybackMessage(type, payload) {
    console.log("[RemotePlayback] Received:", type, payload);

    switch (type) {
      case "playback.welcome":
        handleWelcome(payload);
        break;
      case "playback.state":
        handleRemoteState(payload);
        break;
      case "playback.queue_sync":
        handleQueueSync(payload);
        break;
      case "playback.queue_update":
        handleQueueSync(payload); // Same format as queue_sync
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

    // Start interpolation if there's an active session and we're not the audio device
    if (payload.session.exists && !isAudioDevice.value) {
      startInterpolation();
    }

    // Check if we should reclaim audio device status
    if (reclaimable.value) {
      const player = usePlayerStore();
      if (player.currentTrackId) {
        reclaimAudioDevice();
      }
    }

    console.log("[RemotePlayback] Welcome received, device ID:", deviceId.value);
  }

  function handleRemoteState(payload) {
    if (isAudioDevice.value) return; // Ignore if we're the audio device

    remoteState.value = payload;

    // Check queue version
    if (payload.queue_version > remoteQueueVersion.value) {
      requestQueueSync();
    }
  }

  function handleQueueSync(payload) {
    remoteQueue.value = payload.queue;
    remoteQueueVersion.value = payload.queue_version;
  }

  function handleSessionEnded(payload) {
    console.log("[RemotePlayback] Session ended:", payload?.reason);
    sessionExists.value = false;
    remoteState.value = null;
    remoteQueue.value = [];
    isAudioDevice.value = false;
    stopInterpolation();
    stopBroadcasting();
  }

  function handleDeviceListChanged(payload) {
    devices.value = payload.devices;
    console.log(
      "[RemotePlayback] Device list changed:",
      payload.change.type,
      payload.change.device_id,
    );
  }

  function handleCommand(payload) {
    if (!isAudioDevice.value) return;

    const player = usePlayerStore();
    const { command, payload: cmdPayload } = payload;

    console.log("[RemotePlayback] Received command:", command, cmdPayload);

    switch (command) {
      case "play":
        player.play();
        break;
      case "pause":
        player.pause();
        break;
      case "seek":
        if (cmdPayload?.position !== undefined) {
          // cmdPayload.position is in seconds, seekToPercentage expects 0-1
          const staticsStore = useStaticsStore();
          const track = staticsStore.tracks[player.currentTrackId];
          const duration = track?.duration || 0;
          if (duration > 0) {
            player.seekToPercentage(cmdPayload.position / duration);
          }
        }
        break;
      case "next":
        player.skipNextTrack();
        break;
      case "prev":
        player.skipPreviousTrack();
        break;
      case "setVolume":
        if (cmdPayload?.volume !== undefined) {
          player.setVolume(cmdPayload.volume);
        }
        break;
      case "setMuted":
        if (cmdPayload?.muted !== undefined) {
          player.setMuted(cmdPayload.muted);
        }
        break;
      default:
        console.warn("[RemotePlayback] Unknown command:", command);
    }
  }

  // Audio device registration
  function registerAsAudioDevice() {
    ws.send("playback.register_audio_device", {});
    isAudioDevice.value = true;
    sessionExists.value = true;
    startBroadcasting();
    stopInterpolation();
    console.log("[RemotePlayback] Registered as audio device");
  }

  function unregisterAsAudioDevice() {
    ws.send("playback.unregister_audio_device", {});
    isAudioDevice.value = false;
    stopBroadcasting();
    console.log("[RemotePlayback] Unregistered as audio device");
  }

  function reclaimAudioDevice() {
    const player = usePlayerStore();
    const state = buildPlaybackState(player);
    ws.send("playback.reclaim_audio_device", state);
    isAudioDevice.value = true;
    startBroadcasting();
    stopInterpolation();
    console.log("[RemotePlayback] Reclaimed audio device status");
  }

  // State broadcasting (called when we're the audio device)
  function startBroadcasting() {
    if (broadcastInterval) return;

    broadcastInterval = setInterval(() => {
      const player = usePlayerStore();
      if (player.isPlaying) {
        broadcastCurrentState();
      }
    }, BROADCAST_INTERVAL_MS);

    // Broadcast immediately
    broadcastCurrentState();
  }

  function stopBroadcasting() {
    if (broadcastInterval) {
      clearInterval(broadcastInterval);
      broadcastInterval = null;
    }
  }

  function broadcastCurrentState() {
    if (!isAudioDevice.value) return;

    const player = usePlayerStore();
    const state = buildPlaybackState(player);
    ws.send("playback.state", state);
  }

  function broadcastQueueUpdate() {
    if (!isAudioDevice.value) return;

    const player = usePlayerStore();
    queueVersion.value++;
    const queue = buildQueueItems(player);
    ws.send("playback.queue_update", {
      queue,
      queue_version: queueVersion.value,
    });
  }

  // Remote commands (when not audio device)
  function sendCommand(command, payload = null) {
    ws.send("playback.command", { command, payload });
  }

  function requestQueueSync() {
    ws.send("playback.request_queue", {});
  }

  // Transfer
  function requestBecomeAudioDevice() {
    const transferId = crypto.randomUUID();
    pendingTransfer.value = { transferId, type: "requesting" };
    ws.send("playback.command", {
      command: "becomeAudioDevice",
      payload: { transfer_id: transferId },
    });
    console.log("[RemotePlayback] Requesting to become audio device");
  }

  function handlePrepareTransfer(payload) {
    // We're the current audio device, prepare to transfer
    const player = usePlayerStore();
    const state = buildPlaybackState(player);
    const queue = buildQueueItems(player);

    player.pause(); // Pause before transfer

    ws.send("playback.transfer_ready", {
      transfer_id: payload.transfer_id,
      state,
      queue,
    });

    pendingTransfer.value = {
      transferId: payload.transfer_id,
      type: "source",
    };
    console.log(
      "[RemotePlayback] Preparing transfer to",
      payload.target_device_name,
    );
  }

  async function handleBecomeAudioDevice(payload) {
    // We're becoming the new audio device
    const player = usePlayerStore();
    const staticsStore = useStaticsStore();

    // Apply received state
    await applyRemoteStateToPlayer(player, staticsStore, payload.state, payload.queue);

    // Confirm transfer
    ws.send("playback.transfer_complete", { transfer_id: payload.transfer_id });

    isAudioDevice.value = true;
    sessionExists.value = true;
    pendingTransfer.value = null;
    startBroadcasting();
    stopInterpolation();

    // Start playback if it was playing
    if (payload.state.is_playing) {
      player.play();
    }

    console.log("[RemotePlayback] Became audio device via transfer");
  }

  // eslint-disable-next-line no-unused-vars
  function handleTransferComplete(payload) {
    // Old audio device - transfer succeeded
    const player = usePlayerStore();
    player.stop(); // Fully stop local playback

    isAudioDevice.value = false;
    pendingTransfer.value = null;
    stopBroadcasting();
    startInterpolation();

    console.log("[RemotePlayback] Transfer complete, stopped local playback");
  }

  function handleTransferAborted(payload) {
    console.log("[RemotePlayback] Transfer aborted:", payload.reason);
    pendingTransfer.value = null;

    // If we were source, resume playback
    if (isAudioDevice.value) {
      const player = usePlayerStore();
      player.play();
    }
  }

  // Position interpolation for remote clients
  function startInterpolation() {
    if (interpolationFrame) return;

    const tick = () => {
      if (remoteState.value?.is_playing) {
        const elapsed = (Date.now() - remoteState.value.timestamp) / 1000;
        interpolatedPosition.value = remoteState.value.position + elapsed;
      } else if (remoteState.value) {
        interpolatedPosition.value = remoteState.value.position;
      }
      interpolationFrame = requestAnimationFrame(tick);
    };
    tick();
  }

  function stopInterpolation() {
    if (interpolationFrame) {
      cancelAnimationFrame(interpolationFrame);
      interpolationFrame = null;
    }
  }

  // Helpers
  function buildPlaybackState(player) {
    const staticsStore = useStaticsStore();
    let currentTrack = null;

    if (player.currentTrackId) {
      const track = staticsStore.tracks[player.currentTrackId];
      const album = track?.album_id ? staticsStore.albums[track.album_id] : null;
      const artist = track?.artist_id
        ? staticsStore.artists[track.artist_id]
        : null;

      if (track) {
        currentTrack = {
          id: track.id,
          title: track.title,
          artist_id: track.artist_id || "",
          artist_name: artist?.name || "Unknown Artist",
          album_id: track.album_id || "",
          album_title: album?.name || "Unknown Album",
          duration: track.duration || 0,
          track_number: track.track_number,
          image_id: album?.image_id || null,
        };
      }
    }

    return {
      current_track: currentTrack,
      queue_position: player.currentTrackIndex || 0,
      queue_version: queueVersion.value,
      position: player.progressSec || 0,
      is_playing: player.isPlaying,
      volume: player.volume,
      shuffle: false, // Not implemented in player yet
      repeat: "off", // Not implemented in player yet
      timestamp: Date.now(),
    };
  }

  function buildQueueItems(player) {
    if (!player.currentPlaylist?.tracksIds) {
      return [];
    }
    return player.currentPlaylist.tracksIds.map((id) => ({
      id,
      added_at: Date.now(),
    }));
  }

  async function applyRemoteStateToPlayer(player, staticsStore, state, queue) {
    // Build a playlist from the queue
    if (!queue || queue.length === 0) {
      return;
    }

    const trackIds = queue.map((item) => item.id);

    // Wait for tracks to be loaded first
    for (const id of trackIds) {
      await staticsStore.waitTrackData(id);
    }

    // Set the playlist using the proper method
    const startIndex =
      state.queue_position >= 0 && state.queue_position < trackIds.length
        ? state.queue_position
        : 0;
    player.setPlaylistFromTrackIds(trackIds, startIndex, false);

    // Seek to position after track loads
    if (state.position > 0) {
      // Need to wait for track to load
      setTimeout(() => {
        const track = staticsStore.tracks[trackIds[startIndex]];
        if (track?.duration) {
          player.seekToPercentage(state.position / track.duration);
        }
      }, 500);
    }

    // Apply volume
    if (state.volume !== undefined) {
      player.setVolume(state.volume);
    }
  }

  // Watch for WebSocket connection to initialize
  watch(
    () => ws.wsConnected.value,
    (connected) => {
      if (connected && !initialized.value) {
        initialize();
      } else if (!connected) {
        // Reset state on disconnect
        initialized.value = false;
        deviceId.value = null;
        devices.value = [];
        stopBroadcasting();
        stopInterpolation();
      }
    },
    { immediate: true },
  );

  // Watch for playback changes to broadcast when audio device
  watch(
    () => {
      const player = usePlayerStore();
      return {
        isPlaying: player.isPlaying,
        trackId: player.currentTrackId,
        trackIndex: player.currentTrackIndex,
      };
    },
    () => {
      if (isAudioDevice.value) {
        broadcastCurrentState();
      }
    },
  );

  // Watch for playlist changes to broadcast queue updates
  watch(
    () => {
      const player = usePlayerStore();
      return player.currentPlaylist?.tracksIds?.length;
    },
    () => {
      if (isAudioDevice.value) {
        broadcastQueueUpdate();
      }
    },
  );

  // Cleanup on unmount
  function cleanup() {
    stopBroadcasting();
    stopInterpolation();
    ws.unregisterHandler("playback");
  }

  return {
    // State
    deviceId,
    devices,
    isAudioDevice,
    sessionExists,
    reclaimable,
    remoteState,
    remoteQueue,
    pendingTransfer,
    interpolatedPosition,

    // Computed
    audioDevice,
    otherDevices,
    isRemoteMode,

    // Actions
    initialize,
    registerAsAudioDevice,
    unregisterAsAudioDevice,
    broadcastCurrentState,
    broadcastQueueUpdate,
    sendCommand,
    requestBecomeAudioDevice,
    cleanup,
  };
});
