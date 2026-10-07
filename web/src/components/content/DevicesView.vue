<template>
  <div class="devicesPage">
    <header class="pageHeader">
      <h1 class="pageTitle">Devices</h1>
      <p>See what’s playing and control your connected devices.</p>
    </header>

    <div v-if="allDevices.length === 0" class="emptyState">
      No devices connected.
    </div>

    <div v-if="myDevices.length > 0" class="sectionHeader">Your devices</div>
    <div v-if="myDevices.length > 0" class="deviceCards">
      <div
        v-for="device in myDevices"
        :key="device.id"
        class="deviceCard"
        :class="{
          thisDevice: device.isThisDevice,
          playing: device.isThisDevice
            ? playback.isPlaying
            : device.state?.is_playing,
        }"
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
          <span class="deviceName">{{ device.name }}</span
          ><span
            v-if="
              device.isThisDevice
                ? playback.isPlaying
                : device.state?.is_playing
            "
            class="playingLabel"
            >Playing</span
          >
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
            <button
              type="button"
              class="controlBtn scaleClickFeedback"
              @click="sendCmd('prev', device.id)"
              title="Previous"
            >
              <SkipPrevious />
            </button>
            <button
              type="button"
              class="controlBtn playPauseBtn scaleClickFeedback"
              @click="
                sendCmd(device.state.is_playing ? 'pause' : 'play', device.id)
              "
              :title="device.state.is_playing ? 'Pause' : 'Play'"
            >
              <svg
                v-if="device.state.is_playing"
                viewBox="0 0 24 24"
                aria-hidden="true"
              >
                <path d="M7 5h4v14H7zm6 0h4v14h-4z" />
              </svg>
              <svg v-else viewBox="0 0 24 24" aria-hidden="true">
                <path d="m8 5 11 7-11 7z" />
              </svg>
            </button>
            <button
              type="button"
              class="controlBtn scaleClickFeedback"
              @click="sendCmd('next', device.id)"
              title="Next"
            >
              <SkipNext />
            </button>
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
      Shared devices
    </div>
    <div v-if="sharedDevices.length > 0" class="deviceCards">
      <div
        v-for="device in sharedDevices"
        :key="device.id"
        class="deviceCard"
        :class="{ playing: device.state?.is_playing }"
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
          <span class="deviceName">{{ device.name }}</span
          ><span
            v-if="
              device.isThisDevice
                ? playback.isPlaying
                : device.state?.is_playing
            "
            class="playingLabel"
            >Playing</span
          >
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
          <button
            type="button"
            class="controlBtn scaleClickFeedback"
            @click="sendCmd('prev', device.id)"
            title="Previous"
          >
            <SkipPrevious />
          </button>
          <button
            type="button"
            class="controlBtn playPauseBtn scaleClickFeedback"
            @click="
              sendCmd(device.state.is_playing ? 'pause' : 'play', device.id)
            "
            :title="device.state.is_playing ? 'Pause' : 'Play'"
          >
            <svg
              v-if="device.state.is_playing"
              viewBox="0 0 24 24"
              aria-hidden="true"
            >
              <path d="M7 5h4v14H7zm6 0h4v14h-4z" />
            </svg>
            <svg v-else viewBox="0 0 24 24" aria-hidden="true">
              <path d="m8 5 11 7-11 7z" />
            </svg>
          </button>
          <button
            type="button"
            class="controlBtn scaleClickFeedback"
            @click="sendCmd('next', device.id)"
            title="Next"
          >
            <SkipNext />
          </button>
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
    <div v-if="policyLoaded || policyError" class="sharePolicyCard">
      <div class="sharePolicyHeader">
        <span class="sectionTitle">Sharing this device</span>
        <span v-if="policySaving" class="policyStatus">Saving…</span>
        <span v-if="policyError" class="policyError">{{ policyError }}</span>
      </div>
      <p class="sectionDescription">
        Choose who can control playback on this device.
      </p>
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
          <label for="allow-users">Allow users (IDs, comma separated)</label>
          <input
            id="allow-users"
            v-model="policyState.allowUsers"
            type="text"
            placeholder="e.g. 12, 34"
          />
        </div>
        <div class="policyField">
          <label for="deny-users">Deny users (IDs, comma separated)</label>
          <input
            id="deny-users"
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
          Save sharing settings
        </button>
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
  color: var(--text-base);
}
.pageHeader {
  padding: 16px 0 8px;
}
.pageTitle {
  margin: 0 0 12px;
  font-size: clamp(32px, 4cqw, 48px);
  font-weight: 750;
  letter-spacing: -0.035em;
  line-height: 1.1;
}
.pageHeader p,
.sectionDescription {
  margin: 0;
  color: var(--text-subdued);
  font-size: 14px;
  line-height: 1.6;
}
.sectionHeader {
  margin: 8px 0 -8px;
  font-size: 24px;
  font-weight: 700;
  letter-spacing: -0.02em;
}
.deviceCards {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.deviceCard {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 16px 24px;
  padding: 20px;
  border-radius: 8px;
  background: #181818;
  min-width: 0;
}
.deviceHeader {
  grid-column: 1 / -1;
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 12px;
  min-width: 0;
}
.deviceTypeIcon {
  width: 28px;
  height: 28px;
  flex-shrink: 0;
  color: var(--text-subdued);
}
.deviceName {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 16px;
  font-weight: 700;
}
.playing .deviceTypeIcon,
.playing .deviceName,
.playingLabel {
  color: var(--spotify-green);
}
.playingLabel {
  margin-left: auto;
  font-size: 13px;
}
.thisDeviceBadge,
.sharedBadge {
  color: var(--text-subdued);
  font-size: 12px;
}
.playbackInfo {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 0;
}
.albumArt {
  width: 48px;
  height: 48px;
  flex-shrink: 0;
  border-radius: 4px;
  overflow: hidden;
  background: #282828;
}
.trackDetails {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
}
.trackTitle,
.trackArtist {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.trackTitle {
  font-size: 14px;
  font-weight: 500;
}
.trackArtist {
  font-size: 13px;
  color: var(--text-subdued);
}
.controlsRow {
  display: flex;
  align-items: center;
  gap: 12px;
}
.controlBtn {
  display: grid;
  place-items: center;
  width: 32px;
  height: 32px;
  padding: 0;
  border: 0;
  border-radius: 50%;
  background: transparent;
  color: var(--text-subdued);
  cursor: pointer;
}
.controlBtn:hover {
  color: #fff;
}
.controlBtn svg {
  width: 20px;
  height: 20px;
  fill: currentColor;
}
.playPauseBtn {
  background: #fff;
  color: #000;
}
.playPauseBtn:hover {
  background: #fff;
  color: #000;
  transform: scale(1.06);
}
.progressRow {
  grid-column: 1 / -1;
  display: flex;
  align-items: center;
  gap: 16px;
}
.deviceProgressBar {
  flex: 1;
  min-width: 0;
}
.progressTime {
  color: var(--text-subdued);
  font-size: 12px;
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}
.notPlaying {
  color: var(--text-subdued);
  font-size: 14px;
  grid-column: 1 / -1;
}
.emptyState {
  padding: 48px 24px;
  text-align: center;
  color: var(--text-subdued);
  background: #181818;
  border-radius: 8px;
}
.sharePolicyCard {
  display: flex;
  flex-direction: column;
  gap: 20px;
  margin-top: 16px;
  padding-top: 28px;
  border-top: 1px solid var(--surface-border);
}
.sharePolicyHeader {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 12px;
}
.sectionTitle {
  font-size: 24px;
  font-weight: 700;
  letter-spacing: -0.02em;
}
.policyStatus {
  font-size: 13px;
  color: var(--text-subdued);
}
.policyError {
  font-size: 13px;
  color: #f3727f;
}
.policyModeRow,
.policyRoleRow {
  display: flex;
  flex-wrap: wrap;
  gap: 12px 24px;
}
.policyOption {
  display: inline-flex;
  align-items: center;
  gap: 10px;
  min-height: 40px;
  font-size: 14px;
  cursor: pointer;
}
.policyOption input {
  accent-color: var(--spotify-green);
  width: 18px;
  height: 18px;
  margin: 0;
}
.policyRules {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 20px;
}
.policyField {
  display: flex;
  flex-direction: column;
  gap: 10px;
  min-width: 0;
  font-size: 14px;
}
.policyField:last-child {
  grid-column: 1 / -1;
}
.policyField input[type="text"] {
  min-width: 0;
  width: 100%;
  min-height: 44px;
  padding: 10px 12px;
  border: 1px solid #727272;
  border-radius: 4px;
  background: #242424;
  color: var(--text-base);
  font: inherit;
}
.policyField input[type="text"]:focus {
  outline: 2px solid white;
  outline-offset: -2px;
}
.policyActions {
  display: flex;
  justify-content: flex-end;
}
.primaryBtn {
  min-height: 48px;
  padding: 0 24px;
  border: 0;
  border-radius: 999px;
  background: var(--spotify-green);
  color: #000;
  font-weight: 700;
  font-size: 14px;
  cursor: pointer;
}
.primaryBtn:hover:not(:disabled) {
  background: var(--spotify-green-hover);
  transform: scale(1.02);
}
.primaryBtn:disabled {
  opacity: 0.5;
  cursor: default;
}
button:focus-visible,
input:focus-visible {
  outline: 2px solid #fff;
  outline-offset: 3px;
}
@container (max-width: 550px) {
  .deviceCard {
    padding: 16px;
    gap: 16px;
  }
  .playbackInfo {
    grid-column: 1 / -1;
  }
  .controlsRow {
    grid-column: 1 / -1;
    justify-content: center;
  }
  .policyRules {
    grid-template-columns: 1fr;
  }
  .deviceName {
    max-width: calc(100% - 48px);
  }
  .progressRow {
    flex-wrap: wrap;
    gap: 8px;
  }
  .deviceProgressBar {
    flex-basis: 100%;
  }
}
</style>
