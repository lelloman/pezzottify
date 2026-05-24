<template>
  <div class="devicesPage">
    <h1 class="pageTitle">Devices</h1>

    <div v-if="policyLoaded || policyError" class="sharePolicyCard">
      <div class="sharePolicyHeader">
        <span class="sectionTitle">Device Sharing (This Device)</span>
        <span v-if="policySaving" class="policyStatus">Saving…</span>
        <span v-if="policyError" class="policyError">{{ policyError }}</span>
      </div>
      <div class="policyModeRow">
        <label class="policyOption">
          <input
            type="radio"
            value="deny_everyone"
            v-model="policyState.mode"
          />
          <span>Deny everyone</span>
        </label>
        <label class="policyOption">
          <input
            type="radio"
            value="allow_everyone"
            v-model="policyState.mode"
          />
          <span>Allow everyone</span>
        </label>
        <label class="policyOption">
          <input type="radio" value="custom" v-model="policyState.mode" />
          <span>Custom</span>
        </label>
      </div>

      <div v-if="policyState.mode === 'custom'" class="policyRules">
        <div class="policyField">
          <label>Allow users (IDs, comma separated)</label>
          <input
            v-model="policyState.allowUsers"
            type="text"
            placeholder="e.g. 12, 34"
          />
        </div>
        <div class="policyField">
          <label>Deny users (IDs, comma separated)</label>
          <input
            v-model="policyState.denyUsers"
            type="text"
            placeholder="e.g. 56"
          />
        </div>
        <div class="policyField">
          <label>Allow roles</label>
          <div class="policyRoleRow">
            <label class="policyOption">
              <input type="checkbox" v-model="policyState.allowRoles.admin" />
              <span>Admin</span>
            </label>
            <label class="policyOption">
              <input type="checkbox" v-model="policyState.allowRoles.regular" />
              <span>Regular</span>
            </label>
          </div>
        </div>
      </div>

      <div class="policyActions">
        <button class="primaryBtn" @click="savePolicy" :disabled="policySaving">
          Save Policy
        </button>
      </div>
    </div>

    <div v-if="allDevices.length === 0" class="emptyState">
      No devices connected.
    </div>

    <div v-if="myDevices.length > 0" class="sectionHeader">Your Devices</div>
    <div v-if="myDevices.length > 0" class="deviceCards">
      <div
        v-for="device in myDevices"
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
          <span v-if="device.is_shared" class="sharedBadge">
            shared by {{ device.owner_handle || "unknown" }}
          </span>
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
              @update:progress="(p) => onRemoteSeek(p, device.id, device.state)"
            />
            <span class="progressTime"
              >{{
                formatSec(
                  interpolatedRemotePositionSec(device.id, device.state),
                )
              }}
              / {{ formatMs(device.state.current_track.duration) }}</span
            >
          </div>
          <div v-if="!device.state?.current_track" class="notPlaying">
            Not playing
          </div>
        </template>
      </div>
    </div>

    <div v-if="sharedDevices.length > 0" class="sectionHeader">
      Shared Devices
    </div>
    <div v-if="sharedDevices.length > 0" class="deviceCards">
      <div v-for="device in sharedDevices" :key="device.id" class="deviceCard">
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
          <span v-if="device.is_shared" class="sharedBadge">
            shared by {{ device.owner_handle || "unknown" }}
          </span>
        </div>

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
            @update:progress="(p) => onRemoteSeek(p, device.id, device.state)"
          />
          <span class="progressTime"
            >{{
              formatSec(interpolatedRemotePositionSec(device.id, device.state))
            }}
            / {{ formatMs(device.state.current_track.duration) }}</span
          >
        </div>
        <div v-if="!device.state?.current_track" class="notPlaying">
          Not playing
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import {
  computed,
  ref,
  onMounted,
  onActivated,
  onDeactivated,
  watch,
} from "vue";
import axios from "axios";
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

const policyState = ref({
  mode: "deny_everyone",
  allowUsers: "",
  denyUsers: "",
  allowRoles: {
    admin: false,
    regular: false,
  },
});
const policyLoaded = ref(false);
const policySaving = ref(false);
const policyError = ref("");

function normalizeIdList(input) {
  return input
    .split(",")
    .map((s) => s.trim())
    .filter((s) => s.length > 0)
    .map((s) => Number(s))
    .filter((n) => Number.isFinite(n) && n > 0);
}

function applyPolicyResponse(policy) {
  policyState.value.mode = policy.mode || "deny_everyone";
  policyState.value.allowUsers = (policy.allow_users || []).join(", ");
  policyState.value.denyUsers = (policy.deny_users || []).join(", ");
  const roles = new Set(policy.allow_roles || []);
  policyState.value.allowRoles.admin = roles.has("admin");
  policyState.value.allowRoles.regular = roles.has("regular");
}

async function loadPolicy() {
  if (!sessionStore.myDeviceId) return;
  policyError.value = "";
  try {
    const res = await axios.get("/v1/user/devices");
    const devices = res.data?.devices || [];
    const me = devices.find((d) => d.id === sessionStore.myDeviceId);
    if (me?.share_policy) {
      applyPolicyResponse(me.share_policy);
      policyLoaded.value = true;
    } else {
      policyLoaded.value = false;
    }
  } catch (err) {
    console.error("[Devices] Failed to load share policy", err);
    policyError.value = "Failed to load policy";
  }
}

async function savePolicy() {
  if (!sessionStore.myDeviceId) return;
  policySaving.value = true;
  policyError.value = "";
  try {
    const body = {
      mode: policyState.value.mode,
      allow_users:
        policyState.value.mode === "custom"
          ? normalizeIdList(policyState.value.allowUsers)
          : [],
      deny_users:
        policyState.value.mode === "custom"
          ? normalizeIdList(policyState.value.denyUsers)
          : [],
      allow_roles:
        policyState.value.mode === "custom"
          ? [
              ...(policyState.value.allowRoles.admin ? ["admin"] : []),
              ...(policyState.value.allowRoles.regular ? ["regular"] : []),
            ]
          : [],
    };
    const res = await axios.put(
      `/v1/user/devices/${sessionStore.myDeviceId}/share_policy`,
      body,
    );
    applyPolicyResponse(res.data || body);
  } catch (err) {
    console.error("[Devices] Failed to save share policy", err);
    policyError.value = "Failed to save policy";
  } finally {
    policySaving.value = false;
  }
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

onActivated(() => {
  if (!interpolationTimer) {
    interpolationTimer = setInterval(() => {
      tickCount.value++;
    }, 500);
  }
});

onDeactivated(() => {
  if (interpolationTimer) {
    clearInterval(interpolationTimer);
    interpolationTimer = null;
  }
});

watch(
  () => sessionStore.myDeviceId,
  (deviceId) => {
    if (deviceId) {
      loadPolicy();
    }
  },
  { immediate: true },
);

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

const myDevices = computed(() => allDevices.value.filter((d) => !d.is_shared));
const sharedDevices = computed(() =>
  allDevices.value.filter((d) => d.is_shared),
);
</script>

<style scoped>
.devicesPage {
  display: flex;
  flex-direction: column;
  gap: 24px;
  width: 100%;
  min-height: 100%;
  padding: clamp(18px, 2vw, 30px);
  color: var(--text-base);
}

.pageTitle {
  margin: 0;
  color: #9eddb7;
  font-size: clamp(1.25rem, 1.8vw, 1.65rem);
  font-weight: 900;
  line-height: 1.1;
  text-transform: uppercase;
}

.emptyState {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 140px;
  border: 1px dashed var(--surface-border);
  border-radius: 8px;
  color: var(--text-subdued);
  font-size: 0.9rem;
  font-weight: 700;
}

.sectionHeader {
  margin: 8px 0 -10px;
  color: #9eddb7;
  font-size: clamp(1rem, 1.35vw, 1.32rem);
  font-weight: 900;
  line-height: 1.15;
  text-transform: uppercase;
}

.sharePolicyCard {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 16px;
  border: 1px solid var(--surface-border);
  border-radius: 8px;
  background: var(--surface-panel);
}

.sharePolicyHeader {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
}

.sectionTitle {
  color: #9eddb7;
  font-size: 0.82rem;
  font-weight: 900;
  letter-spacing: 0;
  text-transform: uppercase;
}

.policyStatus {
  color: var(--text-subdued);
  font-size: 0.76rem;
  font-weight: 700;
}

.policyError {
  color: #ffb4a8;
  font-size: 0.76rem;
  font-weight: 750;
}

.policyModeRow,
.policyRoleRow {
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
}

.policyOption {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  min-height: 34px;
  padding: 0 10px;
  border: 1px solid var(--surface-border);
  border-radius: 7px;
  background: rgba(255, 255, 255, 0.035);
  color: var(--text-base);
  font-size: 0.84rem;
  font-weight: 750;
}

.policyOption input {
  accent-color: var(--spotify-green);
}

.policyRules {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 14px;
  padding-top: 2px;
}

.policyField {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 8px;
  color: rgba(255, 255, 255, 0.68);
  font-size: 0.82rem;
  font-weight: 700;
}

.policyField:last-child {
  grid-column: 1 / -1;
}

.policyField input {
  min-height: 38px;
  padding: 0 10px;
  border: 1px solid var(--surface-border);
  border-radius: 7px;
  background: rgba(255, 255, 255, 0.045);
  color: var(--text-base);
}

.policyField input:focus {
  outline: 2px solid var(--spotify-green);
  outline-offset: 1px;
}

.policyActions {
  display: flex;
  justify-content: flex-end;
}

.primaryBtn {
  min-height: 38px;
  padding: 0 16px;
  border: none;
  border-radius: 999px;
  background-color: var(--spotify-green);
  color: #071108;
  cursor: pointer;
  font-size: 0.86rem;
  font-weight: 850;
}

.primaryBtn:hover:not(:disabled) {
  background-color: var(--spotify-green-hover);
}

.primaryBtn:disabled {
  cursor: not-allowed;
  opacity: 0.6;
}

.deviceCards {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
  gap: 14px;
}

.deviceCard {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 14px;
  padding: 14px;
  border: 1px solid var(--surface-border);
  border-radius: 8px;
  background: var(--surface-panel);
}

.deviceCard.thisDevice {
  border-color: rgba(29, 185, 84, 0.5);
  box-shadow: inset 0 0 0 1px rgba(29, 185, 84, 0.16);
}

.deviceHeader {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.deviceTypeIcon {
  flex: 0 0 auto;
  color: var(--text-subdued);
}

.thisDevice .deviceTypeIcon {
  color: var(--spotify-green);
}

.deviceName {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text-base);
  font-size: 0.96rem;
  font-weight: 850;
}

.thisDeviceBadge,
.sharedBadge {
  flex: 0 0 auto;
  padding: 2px 8px;
  border-radius: 999px;
  font-size: 0.68rem;
  font-weight: 850;
}

.thisDeviceBadge {
  background-color: rgba(29, 185, 84, 0.16);
  color: var(--spotify-green);
}

.sharedBadge {
  max-width: 150px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  background-color: rgba(255, 255, 255, 0.07);
  color: var(--text-subdued);
}

.playbackInfo {
  display: grid;
  grid-template-columns: 52px minmax(0, 1fr);
  align-items: center;
  gap: 12px;
}

.albumArt {
  width: 52px;
  height: 52px;
  min-width: 52px;
  overflow: hidden;
  border-radius: 7px;
  background: #242424;
}

.trackDetails {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 3px;
}

.trackTitle {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text-base);
  font-size: 0.9rem;
  font-weight: 850;
}

.trackArtist {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: rgba(255, 255, 255, 0.58);
  font-size: 0.76rem;
  font-weight: 620;
}

.controlsRow {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
}

.controlBtn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border-radius: 8px;
  color: var(--text-subdued);
  cursor: pointer;
  transition:
    color var(--transition-fast),
    background-color var(--transition-fast);
}

.controlBtn:hover {
  background-color: var(--surface-hover);
  color: var(--text-base);
}

.controlBtn svg {
  width: 20px;
  height: 20px;
  fill: currentColor;
}

.playPauseBtn {
  width: 38px;
  height: 38px;
  border-radius: 50%;
  background-color: var(--spotify-green);
  color: #071108;
}

.playPauseBtn:hover {
  background-color: var(--spotify-green-hover);
  color: #071108;
}

.playPauseBtn svg {
  width: 22px;
  height: 22px;
}

.progressRow {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: 12px;
}

.deviceProgressBar {
  min-width: 0;
}

.progressTime {
  min-width: 94px;
  color: var(--text-subdued);
  font-size: 0.72rem;
  font-variant-numeric: tabular-nums;
  font-weight: 700;
  text-align: right;
  white-space: nowrap;
}

.notPlaying {
  display: flex;
  align-items: center;
  min-height: 52px;
  padding: 0 12px;
  border: 1px dashed var(--surface-border);
  border-radius: 8px;
  color: var(--text-subdued);
  font-size: 0.84rem;
  font-weight: 700;
}

@media (max-width: 720px) {
  .devicesPage {
    padding: 14px;
    gap: 18px;
  }

  .policyRules {
    grid-template-columns: 1fr;
  }

  .deviceCards {
    grid-template-columns: 1fr;
  }

  .progressRow {
    grid-template-columns: 1fr;
  }

  .progressTime {
    text-align: left;
  }
}
</style>
