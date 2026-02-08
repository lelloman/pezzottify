<template>
  <div class="devicesPage">
    <h1 class="pageTitle">Your Devices</h1>

    <div v-if="allDevices.length === 0" class="emptyState">
      No devices connected.
    </div>

    <div class="deviceCards">
      <div
        v-for="device in allDevices"
        :key="device.id"
        class="deviceCard"
        :class="{ thisDevice: device.isThisDevice }"
      >
        <div class="deviceHeader">
          <svg
            v-if="device.device_type === 'web'"
            class="deviceTypeIcon"
            viewBox="0 0 24 24"
            fill="currentColor"
            width="18"
            height="18"
          >
            <path
              d="M20 4H4c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V6c0-1.1-.9-2-2-2zm0 14H4V6h16v12z"
            />
          </svg>
          <svg
            v-else
            class="deviceTypeIcon"
            viewBox="0 0 24 24"
            fill="currentColor"
            width="18"
            height="18"
          >
            <path
              d="M16 1H8C6.34 1 5 2.34 5 4v16c0 1.66 1.34 3 3 3h8c1.66 0 3-1.34 3-3V4c0-1.66-1.34-3-3-3zm-2 20h-4v-1h4v1zm3.25-3H6.75V4h10.5v14z"
            />
          </svg>
          <span class="deviceName">{{ device.name }}</span>
          <span v-if="device.isThisDevice" class="thisDeviceBadge"
            >this device</span
          >
        </div>

        <!-- This device: show local playback state -->
        <template v-if="device.isThisDevice">
          <div v-if="localTrack" class="playbackInfo">
            <MultiSourceImage
              :urls="localImageUrls"
              :lazy="false"
              alt="Album art"
              class="albumArt"
            />
            <div class="trackDetails">
              <span class="trackTitle">{{ localTrack.title }}</span>
              <span class="trackArtist">{{ localTrack.artistName }}</span>
            </div>
          </div>
          <div v-if="localTrack" class="progressRow">
            <ProgressBar
              class="deviceProgressBar"
              :progress="playback.progressPercent"
            />
            <span class="progressTime"
              >{{ formatSec(playback.progressSec) }} /
              {{ formatMs(localTrack.duration) }}</span
            >
          </div>
          <div v-if="!localTrack" class="notPlaying">Not playing</div>
        </template>

        <!-- Other device: show remote playback state -->
        <template v-else>
          <div v-if="device.state?.current_track" class="playbackInfo">
            <MultiSourceImage
              :urls="remoteImageUrls(device.state.current_track)"
              :lazy="false"
              alt="Album art"
              class="albumArt"
            />
            <div class="trackDetails">
              <span class="trackTitle">{{
                device.state.current_track.title
              }}</span>
              <span class="trackArtist">{{
                device.state.current_track.artist_name
              }}</span>
            </div>
          </div>
          <div v-if="device.state?.current_track" class="controlsRow">
            <div
              class="controlBtn scaleClickFeedback"
              @click="sendCmd('prev', device.id)"
              title="Previous"
            >
              <SkipPrevious />
            </div>
            <div
              class="controlBtn playPauseBtn scaleClickFeedback"
              @click="
                sendCmd(device.state.is_playing ? 'pause' : 'play', device.id)
              "
              :title="device.state.is_playing ? 'Pause' : 'Play'"
            >
              <PauseIcon v-if="device.state.is_playing" />
              <PlayIcon v-else />
            </div>
            <div
              class="controlBtn scaleClickFeedback"
              @click="sendCmd('next', device.id)"
              title="Next"
            >
              <SkipNext />
            </div>
          </div>
          <div v-if="device.state?.current_track" class="progressRow">
            <ProgressBar
              class="deviceProgressBar"
              :progress="interpolatedRemoteProgress(device.id, device.state)"
              @update:progress="
                (p) => onRemoteSeek(p, device.id, device.state)
              "
            />
            <span class="progressTime"
              >{{ formatSec(interpolatedRemotePositionSec(device.id, device.state)) }} /
              {{ formatMs(device.state.current_track.duration) }}</span
            >
          </div>
          <div v-if="!device.state?.current_track" class="notPlaying">
            Not playing
          </div>
        </template>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed, ref, onMounted, onUnmounted } from "vue";
import { usePlaybackSessionStore } from "@/store/playbackSession";
import { usePlaybackStore } from "@/store/playback";
import MultiSourceImage from "@/components/common/MultiSourceImage.vue";
import ProgressBar from "@/components/common/ProgressBar.vue";
import PlayIcon from "@/components/icons/PlayIcon.vue";
import PauseIcon from "@/components/icons/PauseIcon.vue";
import SkipNext from "@/components/icons/SkipNext.vue";
import SkipPrevious from "@/components/icons/SkipPrevious.vue";

const sessionStore = usePlaybackSessionStore();
const playback = usePlaybackStore();

const localTrack = computed(() => playback.currentTrack);

const localImageUrls = computed(() => {
  if (localTrack.value?.imageId) {
    return [`/v1/content/image/${localTrack.value.imageId}`];
  }
  return [];
});

const remoteImageUrlCache = {};
const EMPTY_URLS = [];
function remoteImageUrls(currentTrack) {
  const id = currentTrack?.image_id;
  if (!id) return EMPTY_URLS;
  if (!remoteImageUrlCache[id]) {
    remoteImageUrlCache[id] = [`/v1/content/image/${id}`];
  }
  return remoteImageUrlCache[id];
}

// ========================================
// Progress interpolation for remote devices
// ========================================

// Tick counter to force reactive updates
const tickCount = ref(0);
let interpolationTimer = null;

onMounted(() => {
  interpolationTimer = setInterval(() => {
    tickCount.value++;
  }, 500);
});

onUnmounted(() => {
  if (interpolationTimer) {
    clearInterval(interpolationTimer);
    interpolationTimer = null;
  }
});

/**
 * Compute the interpolated position in seconds for a remote device.
 * Uses the last received state + elapsed time since the broadcast timestamp.
 */
function interpolatedRemotePositionSec(deviceId, state) {
  // Access tickCount to make this reactive on timer ticks
  void tickCount.value;

  if (!state?.current_track?.duration || state.current_track.duration <= 0)
    return 0;

  const basePosition = state.position || 0;
  const durationSec = state.current_track.duration / 1000;

  if (!state.is_playing || !state.timestamp) {
    return Math.min(basePosition, durationSec);
  }

  // Interpolate: position + elapsed time since broadcast
  const elapsedSec = (Date.now() - state.timestamp) / 1000;
  return Math.min(basePosition + elapsedSec, durationSec);
}

/**
 * Compute the interpolated progress (0-1) for a remote device.
 */
function interpolatedRemoteProgress(deviceId, state) {
  if (!state?.current_track?.duration || state.current_track.duration <= 0)
    return 0;
  const durationSec = state.current_track.duration / 1000;
  if (durationSec <= 0) return 0;
  const posSec = interpolatedRemotePositionSec(deviceId, state);
  return Math.min(posSec / durationSec, 1);
}

function formatSec(sec) {
  const s = Math.floor(sec || 0);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const ss = s % 60;
  const pad = (n) => String(n).padStart(2, "0");
  return `${pad(h)}:${pad(m)}:${pad(ss)}`;
}

function formatMs(ms) {
  return formatSec((ms || 0) / 1000);
}

function sendCmd(command, deviceId) {
  sessionStore.sendCommand(command, {}, deviceId);
}

function onRemoteSeek(progress, deviceId, state) {
  const durationMs = state?.current_track?.duration || 0;
  if (durationMs <= 0) return;
  const positionSec = progress * (durationMs / 1000);
  sessionStore.sendCommand("seek", { position: positionSec }, deviceId);
}

// Build a unified list of all devices with enriched state
const allDevices = computed(() => {
  const myId = sessionStore.myDeviceId;
  const deviceList = sessionStore.devices;
  const otherStates = sessionStore.otherDeviceStates;

  return deviceList.map((d) => ({
    ...d,
    isThisDevice: d.id === myId,
    state: otherStates[d.id]?.state || null,
  }));
});
</script>

<style scoped>
.devicesPage {
  max-width: 640px;
  margin: 0 auto;
}

.pageTitle {
  font-size: var(--text-2xl);
  font-weight: var(--font-bold);
  color: var(--text-base);
  margin: 0 0 var(--spacing-6);
}

.emptyState {
  color: var(--text-subdued);
  font-size: var(--text-base);
}

.deviceCards {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-4);
}

.deviceCard {
  background-color: var(--bg-elevated-base);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-lg);
  padding: var(--spacing-4);
  display: flex;
  flex-direction: column;
  gap: var(--spacing-3);
}

.deviceCard.thisDevice {
  border-color: var(--spotify-green);
  border-width: 2px;
}

.deviceHeader {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
}

.deviceTypeIcon {
  color: var(--text-subdued);
  flex-shrink: 0;
}

.thisDevice .deviceTypeIcon {
  color: var(--spotify-green);
}

.deviceName {
  font-size: var(--text-base);
  font-weight: var(--font-semibold);
  color: var(--text-base);
}

.thisDeviceBadge {
  font-size: var(--text-xs);
  color: var(--spotify-green);
  background-color: rgba(30, 215, 96, 0.1);
  padding: 2px var(--spacing-2);
  border-radius: var(--radius-full);
}

.playbackInfo {
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
}

.albumArt {
  width: 48px;
  height: 48px;
  min-width: 48px;
  border-radius: var(--radius-md);
}

.trackDetails {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.trackTitle {
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
  color: var(--text-base);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.trackArtist {
  font-size: var(--text-xs);
  color: var(--text-subdued);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.controlsRow {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--spacing-2);
}

.controlBtn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  cursor: pointer;
  color: var(--text-base);
  border-radius: var(--radius-full);
  transition:
    color var(--transition-fast),
    background-color var(--transition-fast);
}

.controlBtn:hover {
  color: var(--text-bright);
  background-color: var(--bg-elevated-highlight);
}

.controlBtn svg {
  width: 20px;
  height: 20px;
  fill: currentColor;
}

.playPauseBtn {
  width: 36px;
  height: 36px;
  background-color: var(--spotify-green);
  color: var(--bg-base);
}

.playPauseBtn:hover {
  background-color: var(--spotify-green-hover);
  color: var(--bg-base);
}

.playPauseBtn svg {
  width: 22px;
  height: 22px;
}

.progressRow {
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
}

.deviceProgressBar {
  flex: 1;
  min-width: 0;
}

.progressTime {
  font-size: var(--text-xs);
  color: var(--text-subdued);
  white-space: nowrap;
  font-variant-numeric: tabular-nums;
  min-width: 100px;
  text-align: right;
}

.notPlaying {
  color: var(--text-subdued);
  font-size: var(--text-sm);
  font-style: italic;
}
</style>
