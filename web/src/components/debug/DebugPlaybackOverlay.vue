<script setup>
import { ref, computed, onMounted, onUnmounted } from "vue";
import { useDevicesStore } from "@/store/devices";

const devicesStore = useDevicesStore();
const visible = ref(false);

const hasRemoteState = computed(() => devicesStore.remoteState != null);
const isAudio = computed(() => devicesStore.isAudioDevice);
const sessionActive = computed(() => devicesStore.sessionExists);

// Remote state helpers
const state = computed(() => devicesStore.remoteState);

const trackTitle = computed(() => state.value?.current_track?.title || "-");
const artistName = computed(
  () => state.value?.current_track?.artist_name || "-"
);
const albumTitle = computed(
  () => state.value?.current_track?.album_title || "-"
);
const isPlayingLabel = computed(() =>
  state.value?.is_playing ? "true" : "false"
);
const position = computed(() => formatSeconds(state.value?.position || 0));
const duration = computed(() =>
  formatSeconds(state.value?.current_track?.duration || 0)
);
const volume = computed(() =>
  state.value?.volume != null ? Math.round(state.value.volume * 100) + "%" : "-"
);
const muted = computed(() =>
  state.value?.muted != null ? String(state.value.muted) : "-"
);
const shuffle = computed(() =>
  state.value?.shuffle != null ? String(state.value.shuffle) : "-"
);
const repeat = computed(() => state.value?.repeat || "-");
const queuePosition = computed(() =>
  state.value?.queue_position != null ? state.value.queue_position : "-"
);
const queueVersion = computed(() =>
  state.value?.queue_version != null ? state.value.queue_version : "-"
);
const queueItemCount = computed(() => devicesStore.remoteQueue.length);

// Update timestamp tracking
const lastUpdateTimestamp = ref(null);
const timeSinceUpdate = ref(null);
let updateTimer = null;

// Watch for remote state changes to track timestamps
const updateTimestamp = () => {
  lastUpdateTimestamp.value = Date.now();
};

// Track timestamp from the state itself
const stateTimestamp = computed(() => state.value?.timestamp || null);

// Refresh "time since" every second
function startUpdateTimer() {
  updateTimer = setInterval(() => {
    if (lastUpdateTimestamp.value) {
      timeSinceUpdate.value = Math.round(
        (Date.now() - lastUpdateTimestamp.value) / 1000
      );
    }
  }, 1000);
}

// Watch remoteState changes to capture update times
let prevStateRef = null;
function checkStateChanged() {
  const cur = devicesStore.remoteState;
  if (cur !== prevStateRef) {
    prevStateRef = cur;
    updateTimestamp();
  }
}
let stateCheckTimer = null;

// Device info
const myDeviceId = computed(() => devicesStore.deviceId || "-");
const audioDeviceName = computed(
  () => devicesStore.audioDevice?.name || "-"
);
const audioDeviceId = computed(() => devicesStore.audioDevice?.id || "-");
const deviceCount = computed(() => devicesStore.devices.length);

function formatSeconds(sec) {
  if (sec == null || isNaN(sec)) return "0:00";
  const m = Math.floor(sec / 60);
  const s = Math.floor(sec % 60);
  return `${m}:${s.toString().padStart(2, "0")}`;
}

function handleKeyDown(event) {
  if (event.key === "F2") {
    event.preventDefault();
    visible.value = !visible.value;
  }
}

onMounted(() => {
  window.addEventListener("keydown", handleKeyDown);
  startUpdateTimer();
  stateCheckTimer = setInterval(checkStateChanged, 500);
});

onUnmounted(() => {
  window.removeEventListener("keydown", handleKeyDown);
  if (updateTimer) clearInterval(updateTimer);
  if (stateCheckTimer) clearInterval(stateCheckTimer);
});
</script>

<template>
  <div v-if="visible" class="debug-overlay">
    <div class="debug-header">Playback Debug</div>

    <div class="debug-section">
      <div class="debug-label">Connection</div>
      <div class="debug-row">my device: {{ myDeviceId }}</div>
      <div class="debug-row">devices: {{ deviceCount }}</div>
      <div class="debug-row">session: {{ sessionActive ? "active" : "none" }}</div>
      <div class="debug-row">role: {{ isAudio ? "audio device" : "observer" }}</div>
      <div class="debug-row">audio device: {{ audioDeviceName }}</div>
      <div class="debug-row">audio device id: {{ audioDeviceId }}</div>
    </div>

    <template v-if="hasRemoteState">
      <div class="debug-section">
        <div class="debug-label">Remote State</div>
        <div class="debug-row">track: {{ trackTitle }}</div>
        <div class="debug-row">artist: {{ artistName }}</div>
        <div class="debug-row">album: {{ albumTitle }}</div>
        <div class="debug-row">position: {{ position }} / {{ duration }}</div>
        <div class="debug-row">is_playing: {{ isPlayingLabel }}</div>
        <div class="debug-row">volume: {{ volume }}</div>
        <div class="debug-row">muted: {{ muted }}</div>
        <div class="debug-row">shuffle: {{ shuffle }}</div>
        <div class="debug-row">repeat: {{ repeat }}</div>
      </div>

      <div class="debug-section">
        <div class="debug-label">Queue</div>
        <div class="debug-row">position: {{ queuePosition }}</div>
        <div class="debug-row">version: {{ queueVersion }}</div>
        <div class="debug-row">items: {{ queueItemCount }}</div>
      </div>

      <div class="debug-section">
        <div class="debug-label">Updates</div>
        <div class="debug-row">
          last:
          {{
            lastUpdateTimestamp
              ? new Date(lastUpdateTimestamp).toLocaleTimeString()
              : "-"
          }}
        </div>
        <div class="debug-row">
          ago: {{ timeSinceUpdate != null ? timeSinceUpdate + "s" : "-" }}
        </div>
        <div class="debug-row">
          server ts:
          {{
            stateTimestamp
              ? new Date(stateTimestamp).toLocaleTimeString()
              : "-"
          }}
        </div>
      </div>
    </template>

    <div v-else class="debug-section">
      <div class="debug-no-state">No remote state</div>
    </div>
  </div>
</template>

<style scoped>
.debug-overlay {
  position: fixed;
  top: 12px;
  right: 12px;
  z-index: 9000;
  background: rgba(0, 0, 0, 0.85);
  color: #0f0;
  font-family: "Courier New", Courier, monospace;
  font-size: 11px;
  line-height: 1.4;
  padding: 10px 14px;
  border-radius: 6px;
  border: 1px solid rgba(0, 255, 0, 0.3);
  max-width: 320px;
  pointer-events: none;
  user-select: none;
}

.debug-header {
  font-weight: bold;
  font-size: 12px;
  margin-bottom: 8px;
  color: #0f0;
  border-bottom: 1px solid rgba(0, 255, 0, 0.3);
  padding-bottom: 4px;
}

.debug-section {
  margin-bottom: 6px;
}

.debug-label {
  color: #888;
  font-size: 10px;
  text-transform: uppercase;
  margin-bottom: 2px;
}

.debug-row {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.debug-no-state {
  color: #666;
  font-style: italic;
}
</style>
