/**
 * Remote playback store for multi-device playback sync (Spotify-style).
 *
 * This store manages output device selection and playback state synchronization.
 * The BottomPlayer always acts as the controller - we just select WHERE audio plays.
 *
 * Key concept: selectedOutputDevice
 * - null: Audio plays locally on this device
 * - deviceId: Audio plays on the selected remote device
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
  const initialized = ref(false);

  // Output device selection (null = local, deviceId = remote)
  const selectedOutputDevice = ref(null);

  // Session state from server
  const sessionExists = ref(false);
  const reclaimable = ref(false);

  // Remote state (when outputting to remote device)
  const remoteState = ref(null);
  const remoteQueue = ref([]);
  const remoteQueueVersion = ref(0);

  // Transfer state
  const pendingTransfer = ref(null);

  // Interpolated position (updated via requestAnimationFrame)
  const interpolatedPosition = ref(0);
  let interpolationFrame = null;

  // Broadcast interval (when this device is the output)
  let broadcastInterval = null;
  const BROADCAST_INTERVAL_MS = 5000;

  // Queue version for tracking changes
  const queueVersion = ref(0);

  // Computed
  const isLocalOutput = computed(() => selectedOutputDevice.value === null);

  const currentOutputDevice = computed(() => {
    if (selectedOutputDevice.value === null) {
      return devices.value.find((d) => d.id === deviceId.value);
    }
    return devices.value.find((d) => d.id === selectedOutputDevice.value);
  });

  const otherDevices = computed(() =>
    devices.value.filter((d) => d.id !== deviceId.value),
  );

  // For backward compatibility - audioDevice is the device currently outputting audio
  const audioDevice = computed(() =>
    devices.value.find((d) => d.is_audio_device),
  );

  // Are we the device outputting audio in the session?
  const isAudioDevice = computed(() => {
    const ad = audioDevice.value;
    return ad && ad.id === deviceId.value;
  });

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

      // Set selected output to the current audio device
      const currentAudioDevice = devices.value.find((d) => d.is_audio_device);
      if (currentAudioDevice) {
        if (currentAudioDevice.id === payload.device_id) {
          // We are the audio device
          selectedOutputDevice.value = null;
          startBroadcasting();
        } else {
          // Another device is the audio device
          selectedOutputDevice.value = currentAudioDevice.id;
          startInterpolation();
        }
      }
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
    selectedOutputDevice.value = null;
    stopInterpolation();
    stopBroadcasting();
  }

  function handleDeviceListChanged(payload) {
    devices.value = payload.devices;

    // Update selected output if the audio device changed
    const currentAudioDevice = devices.value.find((d) => d.is_audio_device);
    if (currentAudioDevice) {
      if (currentAudioDevice.id === deviceId.value) {
        selectedOutputDevice.value = null;
      } else if (selectedOutputDevice.value !== currentAudioDevice.id) {
        // Audio device changed to a different device
        selectedOutputDevice.value = currentAudioDevice.id;
      }
    }

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

  // ============================================
  // Unified command interface - routes based on selectedOutputDevice
  // ============================================

  function play() {
    if (isLocalOutput.value) {
      const player = usePlayerStore();
      player.play();
    } else {
      sendCommand("play");
    }
  }

  function pause() {
    if (isLocalOutput.value) {
      const player = usePlayerStore();
      player.pause();
    } else {
      sendCommand("pause");
    }
  }

  function playPause() {
    if (isLocalOutput.value) {
      const player = usePlayerStore();
      player.playPause();
    } else {
      const currentlyPlaying = remoteState.value?.is_playing;
      sendCommand(currentlyPlaying ? "pause" : "play");
    }
  }

  function seek(positionSec) {
    if (isLocalOutput.value) {
      const player = usePlayerStore();
      const staticsStore = useStaticsStore();
      const track = staticsStore.tracks[player.currentTrackId];
      if (track?.duration > 0) {
        player.seekToPercentage(positionSec / track.duration);
      }
    } else {
      sendCommand("seek", { position: positionSec });
    }
  }

  function seekToPercentage(percent) {
    if (isLocalOutput.value) {
      const player = usePlayerStore();
      player.seekToPercentage(percent);
    } else {
      const duration = remoteState.value?.current_track?.duration || 0;
      if (duration > 0) {
        sendCommand("seek", { position: percent * duration });
      }
    }
  }

  function skipNext() {
    if (isLocalOutput.value) {
      const player = usePlayerStore();
      player.skipNextTrack();
    } else {
      sendCommand("next");
    }
  }

  function skipPrevious() {
    if (isLocalOutput.value) {
      const player = usePlayerStore();
      player.skipPreviousTrack();
    } else {
      sendCommand("prev");
    }
  }

  function forward10Sec() {
    if (isLocalOutput.value) {
      const player = usePlayerStore();
      player.forward10Sec();
    } else {
      const currentPos = interpolatedPosition.value || 0;
      sendCommand("seek", { position: currentPos + 10 });
    }
  }

  function rewind10Sec() {
    if (isLocalOutput.value) {
      const player = usePlayerStore();
      player.rewind10Sec();
    } else {
      const currentPos = interpolatedPosition.value || 0;
      sendCommand("seek", { position: Math.max(0, currentPos - 10) });
    }
  }

  function setVolume(volume) {
    if (isLocalOutput.value) {
      const player = usePlayerStore();
      player.setVolume(volume);
    } else {
      sendCommand("setVolume", { volume });
    }
  }

  function setMuted(muted) {
    if (isLocalOutput.value) {
      const player = usePlayerStore();
      player.setMuted(muted);
    } else {
      sendCommand("setMuted", { muted });
    }
  }

  function stop() {
    // Stop only makes sense for local playback
    if (isLocalOutput.value) {
      const player = usePlayerStore();
      player.stop();
    }
    // For remote: ignore (no equivalent remote command)
  }

  // ============================================
  // Output device selection
  // ============================================

  function selectOutputDevice(targetDeviceId) {
    if (targetDeviceId === deviceId.value || targetDeviceId === null) {
      // Select this device as output
      if (!isAudioDevice.value) {
        // Need to transfer playback to this device
        requestBecomeAudioDevice();
      }
      selectedOutputDevice.value = null;
    } else {
      // Select a remote device as output
      const currentAudioDeviceId = audioDevice.value?.id;
      if (currentAudioDeviceId === targetDeviceId) {
        // Already outputting to that device, just update selection
        selectedOutputDevice.value = targetDeviceId;
      } else {
        // Need to transfer playback to the target device
        requestTransferTo(targetDeviceId);
      }
    }
  }

  // ============================================
  // Audio device management (internal)
  // ============================================

  function registerAsAudioDevice() {
    ws.send("playback.register_audio_device", {});
    sessionExists.value = true;
    selectedOutputDevice.value = null;
    startBroadcasting();
    stopInterpolation();
    console.log("[RemotePlayback] Registered as audio device");
  }

  function unregisterAsAudioDevice() {
    ws.send("playback.unregister_audio_device", {});
    stopBroadcasting();
    console.log("[RemotePlayback] Unregistered as audio device");
  }

  function reclaimAudioDevice() {
    const player = usePlayerStore();
    const state = buildPlaybackState(player);
    ws.send("playback.reclaim_audio_device", state);
    selectedOutputDevice.value = null;
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

  function requestTransferTo(targetDeviceId) {
    // Currently we can only request transfer to ourselves
    // To transfer to another device, we'd need a different protocol
    // For now, just update the selection if there's already a session
    if (sessionExists.value) {
      selectedOutputDevice.value = targetDeviceId;
      startInterpolation();
    }
    console.log("[RemotePlayback] Transfer to remote device:", targetDeviceId);
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

    sessionExists.value = true;
    selectedOutputDevice.value = null;
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

    // Update selected output to the new audio device
    const newAudioDevice = devices.value.find((d) => d.is_audio_device);
    if (newAudioDevice) {
      selectedOutputDevice.value = newAudioDevice.id;
    }

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

  // Position interpolation for remote output
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

  // ============================================
  // Unified state getters for UI
  // ============================================

  // Current playback state - combines local and remote
  const currentTrack = computed(() => {
    if (isLocalOutput.value) {
      const player = usePlayerStore();
      const staticsStore = useStaticsStore();
      const track = player.currentTrackId
        ? staticsStore.tracks[player.currentTrackId]
        : null;
      if (!track) return null;

      const album = track.album_id ? staticsStore.albums[track.album_id] : null;
      const artist = track.artist_id
        ? staticsStore.artists[track.artist_id]
        : null;

      return {
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
    return remoteState.value?.current_track || null;
  });

  const currentPosition = computed(() => {
    if (isLocalOutput.value) {
      const player = usePlayerStore();
      return player.progressSec || 0;
    }
    return interpolatedPosition.value || 0;
  });

  const currentProgressPercent = computed(() => {
    if (isLocalOutput.value) {
      const player = usePlayerStore();
      return player.progressPercent || 0;
    }
    const track = remoteState.value?.current_track;
    if (track?.duration > 0) {
      return interpolatedPosition.value / track.duration;
    }
    return 0;
  });

  const isPlaying = computed(() => {
    if (isLocalOutput.value) {
      const player = usePlayerStore();
      return player.isPlaying;
    }
    return remoteState.value?.is_playing || false;
  });

  const currentVolume = computed(() => {
    if (isLocalOutput.value) {
      const player = usePlayerStore();
      return player.volume;
    }
    return remoteState.value?.volume || 1;
  });

  const currentMuted = computed(() => {
    if (isLocalOutput.value) {
      const player = usePlayerStore();
      return player.muted;
    }
    return remoteState.value?.muted || false;
  });

  // ============================================
  // Helpers
  // ============================================

  function buildPlaybackState(player) {
    const staticsStore = useStaticsStore();
    let currentTrackData = null;

    if (player.currentTrackId) {
      const track = staticsStore.tracks[player.currentTrackId];
      const album = track?.album_id ? staticsStore.albums[track.album_id] : null;
      const artist = track?.artist_id
        ? staticsStore.artists[track.artist_id]
        : null;

      if (track) {
        currentTrackData = {
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
      current_track: currentTrackData,
      queue_position: player.currentTrackIndex || 0,
      queue_version: queueVersion.value,
      position: player.progressSec || 0,
      is_playing: player.isPlaying,
      volume: player.volume,
      muted: player.muted || false,
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

    // Store the pending seek position before setting the playlist
    // The player's onload callback will handle seeking when ready
    if (state.position > 0) {
      const track = staticsStore.tracks[trackIds[startIndex]];
      if (track?.duration) {
        // Store pending seek as percentage for player.js to handle on load
        player.setPendingTransferSeek(state.position / track.duration);
      }
    }

    player.setPlaylistFromTrackIds(trackIds, startIndex, false);

    // Apply volume
    if (state.volume !== undefined) {
      player.setVolume(state.volume);
    }

    // Apply muted state if available
    if (state.muted !== undefined) {
      player.setMuted(state.muted);
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
        selectedOutputDevice.value = null;
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
    selectedOutputDevice,
    sessionExists,
    reclaimable,
    remoteState,
    remoteQueue,
    pendingTransfer,
    interpolatedPosition,

    // Computed
    isLocalOutput,
    isAudioDevice,
    currentOutputDevice,
    audioDevice,
    otherDevices,

    // Unified state getters
    currentTrack,
    currentPosition,
    currentProgressPercent,
    isPlaying,
    currentVolume,
    currentMuted,

    // Unified commands (route based on selectedOutputDevice)
    play,
    pause,
    playPause,
    seek,
    seekToPercentage,
    skipNext,
    skipPrevious,
    forward10Sec,
    rewind10Sec,
    setVolume,
    setMuted,
    stop,

    // Output device selection
    selectOutputDevice,

    // Internal (for compatibility with player.js integration)
    registerAsAudioDevice,
    unregisterAsAudioDevice,
    broadcastCurrentState,
    broadcastQueueUpdate,
    sendCommand,
    cleanup,
  };
});
