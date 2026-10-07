<template>
  <footer
    v-if="
      hasPlayback ||
      playback.radioCreationState.status !== 'idle' ||
      playback.radioContinuationError
    "
    class="footerPlayer"
    :class="{
      hasRadioCreation:
        playback.radioCreationState.status !== 'idle' ||
        playback.radioContinuationError,
    }"
  >
    <div
      v-if="playback.radioCreationState.status !== 'idle'"
      class="radioCreationStatus"
      role="status"
      aria-live="polite"
      :aria-busy="playback.radioCreationState.status === 'creating'"
    >
      <i
        v-if="playback.radioCreationState.status === 'creating'"
        class="radioSpinner"
        aria-hidden="true"
      />
      <span>{{
        playback.radioCreationState.status === "creating"
          ? "Creating radio…"
          : playback.radioCreationState.message
      }}</span>
      <button
        v-if="playback.radioCreationState.status === 'error'"
        @click="playback.retryRadioCreation"
      >
        Retry
      </button>
      <button @click="playback.cancelRadioCreation">
        {{
          playback.radioCreationState.status === "creating"
            ? "Cancel"
            : "Dismiss"
        }}
      </button>
    </div>
    <div
      v-if="
        playback.radioContinuationError &&
        playback.radioCreationState.status === 'idle'
      "
      class="radioCreationStatus"
      role="status"
    >
      <span>Could not add more radio tracks.</span>
      <button @click="playback.retryRadioContinuation">Retry</button>
    </div>
    <template v-if="hasPlayback">
      <div class="trackInfoRow">
        <MultiSourceImage
          :urls="imageUrls"
          :lazy="false"
          alt="Image"
          class="trackImage scaleClickFeedback"
          @click.stop="handleClickOnAlbumCover"
        />
        <div class="trackNamesColumn">
          <TrackName
            v-if="displayTrack"
            :track="displayTrack"
            :infiniteAnimation="true"
          />
          <LoadClickableArtistsNames
            v-if="artists.length > 0"
            :artistsIds="artists"
          />
          <span v-else-if="artistName" class="artistName">{{
            artistName
          }}</span>
        </div>
      </div>
      <div class="playerControlsColumn">
        <div class="playerControlsButtonsRow">
          <ControlIconButton
            label="Rewind 10 seconds"
            :action="handleRewind10Sec"
            :icon="Rewind10Sec"
          />
          <ControlIconButton
            label="Previous track"
            :action="handleSkipPrevious"
            :icon="SkipPrevious"
          />
          <button
            type="button"
            class="playPauseButton"
            :aria-label="playback.isPlaying ? 'Pause' : 'Play'"
            :title="playback.isPlaying ? 'Pause' : 'Play'"
            @click="handlePlayPause"
          >
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <path v-if="playback.isPlaying" d="M6 4h4v16H6zM14 4h4v16h-4z" />
              <path v-else d="M7 3v18l15-9z" />
            </svg>
          </button>
          <ControlIconButton
            label="Next track"
            :action="handleSkipNext"
            :icon="NextTrack"
          />
          <ControlIconButton
            label="Forward 10 seconds"
            :action="handleForward10Sec"
            :icon="Forward10Sec"
          />
        </div>
        <div class="progressControlsRow">
          <span>{{ formattedTime }}</span>
          <ProgressBar
            id="TrackProgressBar"
            aria-label="Playback position"
            class="trackProgressBar"
            :progress="combinedProgressPercent"
            @update:progress="updateTrackProgress"
            @update:startDrag="startDraggingTrackProgress"
            @update:stopDrag="handleSeek"
          />
          <span>{{ duration }}</span>
        </div>
      </div>
      <div class="extraControlsRow">
        <ControlIconButton
          v-if="playback.muted"
          label="Unmute"
          :action="handleVolumeOn"
          :icon="VolumeOffIcon"
        />
        <ControlIconButton
          v-if="!playback.muted"
          label="Mute"
          :action="handleVolumeOff"
          :icon="VolumeOnIcon"
        />
        <ProgressBar
          class="volumeProgressBar"
          aria-label="Volume"
          :progress="computedVolumePercent"
          @update:progress="updateVolumeProgress"
          @update:startDrag="startDraggingVolumeProgress"
          @update:stopDrag="handleSetVolume"
        />
        <button
          type="button"
          v-if="
            playback.currentPlaylist?.type !== playback.PLAYBACK_CONTEXTS.radio
          "
          class="playerIconButton smartContinuationButton"
          :class="{ active: smartContinuationEnabled }"
          :title="
            smartContinuationEnabled
              ? 'Smart continuation on'
              : 'Smart continuation off'
          "
          :aria-label="
            smartContinuationEnabled
              ? 'Turn smart continuation off'
              : 'Turn smart continuation on'
          "
          :aria-pressed="smartContinuationEnabled"
          @click="toggleSmartContinuation"
        >
          <AiContinuationIcon />
        </button>
        <button
          type="button"
          v-if="
            playback.currentPlaylist?.type !== playback.PLAYBACK_CONTEXTS.radio
          "
          class="playerIconButton smartContinuationButton"
          :class="{ active: steeringDestination }"
          :title="
            steeringDestination
              ? 'Steering toward ' +
                (steeringDestination.label || steeringDestination.entity_id)
              : 'Steering'
          "
          aria-label="Open steering"
          @click="router.push('/steering')"
        >
          <SteeringWheelIcon />
        </button>
        <button
          type="button"
          class="playerIconButton expandPlayer"
          aria-label="Open now playing and lyrics"
          title="Now playing and lyrics"
          :aria-current="route.name === 'now-playing' ? 'page' : undefined"
          @click="router.push({ name: 'now-playing' })"
        >
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.8"
            aria-hidden="true"
          >
            <path d="M14 4h6v6M20 4l-9 9M10 4H4v16h16v-6" />
          </svg>
        </button>
        <DeviceSelector />
        <ControlIconButton
          v-if="playback.mode === 'local'"
          label="Stop playback"
          :action="handleStop"
          :icon="StopIcon"
        />
      </div>
    </template>
  </footer>
</template>

<script setup>
import { computed, ref, watch, h } from "vue";
import { usePlaybackStore } from "@/store/playback";
import { chooseAlbumCoverImageUrl } from "@/utils";
import Forward10Sec from "./icons/Forward10Sec.vue";
import Rewind10Sec from "./icons/Rewind10Sec.vue";
import NextTrack from "./icons/SkipNext.vue";
import SkipPrevious from "./icons/SkipPrevious.vue";
import ProgressBar from "@/components/common/ProgressBar.vue";
import StopIcon from "./icons/StopIcon.vue";
import VolumeOnIcon from "./icons/VolumeOnIcon.vue";
import VolumeOffIcon from "./icons/VolumeOffIcon.vue";
import MultiSourceImage from "./common/MultiSourceImage.vue";
import LoadClickableArtistsNames from "@/components/common/LoadClickableArtistsNames.vue";
import { useRoute, useRouter } from "vue-router";
import TrackName from "./common/TrackName.vue";
import { useStaticsStore } from "@/store/statics";
import DeviceSelector from "./DeviceSelector.vue";
import AiContinuationIcon from "./icons/AiContinuationIcon.vue";
import SteeringWheelIcon from "./icons/SteeringWheelIcon.vue";
import { useUserStore } from "@/store/user";

const ControlIconButton = {
  props: ["icon", "action", "label"],
  setup(props) {
    const onClick = () => {
      props.action();
    };

    return () =>
      h(
        "button",
        {
          type: "button",
          class: "playerIconButton",
          "aria-label": props.label,
          title: props.label,
          onClick,
        },
        [h(props.icon, { "aria-hidden": "true" })],
      );
  },
};

const router = useRouter();
const route = useRoute();
const playback = usePlaybackStore();
const staticsStore = useStaticsStore();
const userStore = useUserStore();
// Track whether there's any playback to display
const hasPlayback = computed(() => {
  return playback.currentTrackId;
});

// Progress dragging state
const draggingTrackPercent = ref(null);
const combinedProgressPercent = computed(() => {
  return draggingTrackPercent.value ?? playback.progressPercent;
});

// Volume dragging state
const draggingVolumePercent = ref(null);
const computedVolumePercent = computed(() => {
  return (
    draggingVolumePercent.value ?? (playback.muted ? 0.0 : playback.volume)
  );
});

const smartContinuationEnabled = computed(
  () => userStore.isSmartContinuationEnabled,
);
const steeringDestination = computed(
  () => playback.currentGravity?.destination ?? null,
);

// Track display state
const displayTrack = ref(null);
const artists = ref([]);
const artistName = ref(null);
const imageUrls = ref([]);
const duration = ref("");

const formatTime = (timeInSeconds) => {
  const hours = Math.floor(timeInSeconds / 3600);
  const minutes = Math.floor((timeInSeconds % 3600) / 60);
  const seconds = Math.floor(timeInSeconds % 60);

  const pad = (num) => String(num).padStart(2, "0");

  return hours
    ? `${hours}:${pad(minutes)}:${pad(seconds)}`
    : `${minutes}:${pad(seconds)}`;
};

const formattedTime = computed(() => formatTime(playback.progressSec));

const handleClickOnAlbumCover = () => {
  if (displayTrack.value && displayTrack.value.album_id) {
    router.push("/album/" + displayTrack.value.album_id);
  }
};

// ============================================
// Playback controls - all route through playback store
// ============================================

function handlePlayPause() {
  playback.playPause();
}

function handleSkipNext() {
  playback.skipNextTrack();
}

function handleSkipPrevious() {
  playback.skipPreviousTrack();
}

function handleForward10Sec() {
  playback.forward10Sec();
}

function handleRewind10Sec() {
  playback.rewind10Sec();
}

function handleStop() {
  playback.stop();
}

// Progress bar handling
const startDraggingTrackProgress = () => {
  draggingTrackPercent.value = playback.progressPercent;
};

const updateTrackProgress = (event) => {
  draggingTrackPercent.value = event;
};

const handleSeek = () => {
  if (draggingTrackPercent.value !== null) {
    playback.seekToPercentage(draggingTrackPercent.value);
    draggingTrackPercent.value = null;
  }
};

// Volume handling
const startDraggingVolumeProgress = () => {
  draggingVolumePercent.value = playback.volume;
};

const updateVolumeProgress = (event) => {
  draggingVolumePercent.value = event;
};

const handleVolumeOn = () => {
  playback.setMuted(false);
};

const handleVolumeOff = () => {
  playback.setMuted(true);
};

const handleSetVolume = () => {
  if (draggingVolumePercent.value !== null) {
    playback.setVolume(draggingVolumePercent.value);
    playback.setMuted(false);
    draggingVolumePercent.value = null;
  }
};

const toggleSmartContinuation = () => {
  userStore.setSmartContinuationEnabled(!userStore.isSmartContinuationEnabled);
};

// ============================================
// Watch current track from unified store
// ============================================

let trackDataUnwatcher = null;
let albumDataUnwatcher = null;

watch(
  () => playback.currentTrack,
  (track) => {
    // Clean up existing watchers
    if (trackDataUnwatcher) {
      trackDataUnwatcher();
      trackDataUnwatcher = null;
    }
    if (albumDataUnwatcher) {
      albumDataUnwatcher();
      albumDataUnwatcher = null;
    }

    if (!track) {
      displayTrack.value = null;
      artists.value = [];
      artistName.value = null;
      imageUrls.value = [];
      duration.value = "";
      return;
    }

    // In remote mode, track data comes directly from the playback store
    if (playback.mode === "remote") {
      displayTrack.value = {
        id: track.id,
        name: track.title,
        album_id: track.albumId,
        artists_ids: track.artistId ? [track.artistId] : [],
      };
      artists.value = track.artistId ? [track.artistId] : [];
      artistName.value = track.artistName;
      duration.value = track.duration ? formatTime(track.duration / 1000) : "";
      if (track.imageId) {
        imageUrls.value = [`/v1/content/image/${track.imageId}`];
      } else {
        imageUrls.value = [];
      }
      return;
    }

    // Resolve the track data from statics store
    if (playback.currentTrackId) {
      trackDataUnwatcher = watch(
        staticsStore.getTrack(playback.currentTrackId),
        (trackRef) => {
          if (trackRef.item) {
            const localTrack = trackRef.item;
            displayTrack.value = localTrack;
            artists.value = localTrack.artists_ids || [];
            artistName.value = null;
            duration.value = localTrack.duration
              ? formatTime(localTrack.duration / 1000)
              : "";

            // Watch album for cover image
            if (localTrack.album_id && !albumDataUnwatcher) {
              albumDataUnwatcher = watch(
                staticsStore.getAlbum(localTrack.album_id),
                (albumRef) => {
                  if (albumRef && albumRef.item) {
                    imageUrls.value = chooseAlbumCoverImageUrl(albumRef.item);
                  }
                },
                { immediate: true },
              );
            }
          }
        },
        { immediate: true },
      );
    }
  },
  { immediate: true, deep: true },
);
</script>

<style scoped>
.playerIconButton {
  display: grid;
  place-items: center;
  flex-shrink: 0;
  width: 32px;
  height: 32px;
  padding: 8px;
  border: 0;
  border-radius: 50%;
  background: transparent;
  color: var(--text-subdued);
  cursor: pointer;
  transition:
    color 150ms cubic-bezier(0.3, 0, 0, 1),
    transform 150ms cubic-bezier(0.3, 0, 0, 1);
}
.playerIconButton :deep(svg) {
  display: block;
  width: 16px;
  height: 16px;
}
.playerIconButton :deep(svg:not([fill="none"])) {
  fill: currentColor;
}
@media (hover: hover) {
  .playerIconButton:hover {
    color: var(--text-base);
    background: transparent;
    transform: scale(1.04);
    transition-duration: 50ms;
  }
}
.playerIconButton:active {
  background: transparent;
  transform: scale(1);
}
.playerIconButton:focus-visible {
  outline: 2px solid var(--spotify-green);
  outline-offset: 2px;
}

.expandPlayer[aria-current="page"] {
  color: var(--spotify-green);
}

.radioCreationStatus {
  grid-column: 1 / -1;
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 20px;
  background: var(--surface-elevated, #252525);
  color: var(--text-bright, white);
}
.radioCreationStatus span {
  flex: 1;
}
.radioCreationStatus button {
  cursor: pointer;
}
.radioSpinner {
  width: 16px;
  height: 16px;
  flex-shrink: 0;
  border: 2px solid currentColor;
  border-right-color: transparent;
  border-radius: 50%;
  animation: radioSpin 1s linear infinite;
}
@keyframes radioSpin {
  to {
    transform: rotate(360deg);
  }
}
@media (prefers-reduced-motion: reduce) {
  .radioSpinner {
    animation: none;
  }
}
.footerPlayer.hasRadioCreation {
  height: auto;
  min-height: var(--player-height-desktop);
}

/* ============================================
   Footer Player - Desktop Layout
   ============================================ */

.footerPlayer {
  position: relative;
  height: var(--player-height-desktop);
  display: grid;
  grid-template-columns: minmax(240px, 3fr) minmax(320px, 4fr) minmax(
      220px,
      3fr
    );
  gap: 18px;
  padding: 10px 14px;
  align-items: center;
  background: var(--bg-base);
  border-top: 0;
}

/* ============================================
   Track Info Section (Left 30%)
   ============================================ */

.trackInfoRow {
  display: flex;
  flex-direction: row;
  align-items: center;
  gap: 12px;
  min-width: 0;
  text-align: left;
}

.trackImage {
  width: 60px;
  height: 60px;
  min-width: 60px;
  border-radius: 8px;
  cursor: pointer;
  transition:
    transform var(--transition-base),
    box-shadow var(--transition-base);
}

.trackImage:hover {
  transform: scale(1.05);
  box-shadow: var(--shadow-md);
}

.trackImage:active {
  transform: scale(0.98);
}

.trackNamesColumn {
  min-width: 0;
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.trackName {
  margin: 0;
  font-size: var(--font-size-base);
  font-weight: var(--font-semibold);
  color: var(--text-base);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.trackArtist {
  margin: 0;
  font-size: var(--text-sm);
  color: var(--text-subdued);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.artistName {
  font-size: var(--text-sm);
  color: var(--text-subdued);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

/* ============================================
   Player Controls Section (Center 40%)
   ============================================ */

.playerControlsColumn {
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 8px;
  min-width: 0;
}

.playerControlsButtonsRow {
  display: flex;
  flex-direction: row;
  justify-content: center;
  align-items: center;
  gap: 8px;
}

.playPauseButton {
  display: grid;
  place-items: center;
  flex-shrink: 0;
  width: 32px;
  height: 32px;
  padding: 8px;
  margin: 0 8px;
  border: 0;
  border-radius: 50%;
  background: #fff;
  color: #000;
  cursor: pointer;
  transition:
    color 150ms cubic-bezier(0.3, 0, 0, 1),
    transform 150ms cubic-bezier(0.3, 0, 0, 1);
}
.playPauseButton svg {
  width: 16px;
  height: 16px;
  fill: currentColor;
}
.playPauseButton:hover {
  transform: scale(1.04);
  transition-duration: 50ms;
}
.playPauseButton:active {
  transform: scale(1);
}
.playPauseButton:focus-visible {
  outline: 2px solid var(--spotify-green);
  outline-offset: 4px;
}

.progressControlsRow {
  display: flex;
  flex-direction: row;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.progressControlsRow span {
  font-size: var(--text-xs);
  font-weight: var(--font-normal);
  color: var(--text-subdued);
  min-width: 32px;
  text-align: center;
  font-variant-numeric: tabular-nums;
}

.trackProgressBar {
  flex: 1;
  min-width: 0;
}

/* ============================================
   Extra Controls Section (Right 30%)
   ============================================ */

.extraControlsRow {
  display: flex;
  flex-direction: row;
  justify-content: flex-end;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.volumeProgressBar {
  width: 120px;
  max-width: 120px;
}

/* ============================================
   Icon Button Styling
   ============================================ */

.smartContinuationButton {
  position: relative;
  appearance: none;
  border: 0;
  background: transparent;
}

.smartContinuationButton svg {
  width: 16px;
  height: 16px;
  fill: currentColor;
}

.smartContinuationButton.active {
  color: var(--spotify-green);
}

/* ============================================
   Mobile Layout (< 768px)
   ============================================ */

@media (max-width: 767px) {
  .footerPlayer.hasRadioCreation {
    grid-template-rows: 4px 1fr auto auto;
  }
  .hasRadioCreation .radioCreationStatus {
    grid-row: 4;
  }
  .footerPlayer {
    height: var(--player-height-mobile);
    grid-template-columns: minmax(0, 1fr) auto;
    grid-template-rows: 4px 1fr auto;
    gap: 8px;
    padding: 8px 10px;
  }

  .trackInfoRow {
    grid-column: 1;
    grid-row: 2;
    gap: 8px;
  }

  .trackImage {
    width: 48px;
    height: 48px;
    min-width: 48px;
  }

  .trackNamesColumn {
    gap: 2px;
  }

  .trackName {
    font-size: var(--text-sm);
  }

  .trackArtist {
    font-size: var(--text-xs);
  }

  .playerControlsColumn {
    display: contents;
  }
  .playerControlsButtonsRow {
    grid-column: 2;
    grid-row: 2;
  }

  .playerControlsButtonsRow {
    gap: 3px;
  }

  /* Hide skip and seek buttons on mobile */
  .playerControlsButtonsRow
    > :not(:nth-child(3)):not(:nth-child(2)):not(:nth-child(4)) {
    display: none;
  }

  .progressControlsRow {
    grid-column: 1 / -1;
    grid-row: 1;
    gap: 0;
    padding: 0;
  }

  .progressControlsRow span {
    display: none;
  }

  .trackProgressBar {
    width: 100%;
  }

  .extraControlsRow {
    grid-column: 1 / -1;
    grid-row: 3;
    gap: 3px;
  }

  .volumeProgressBar {
    display: none;
  }
}

/* ============================================
   Tablet Layout (768px - 1023px)
   ============================================ */

@media (min-width: 768px) and (max-width: 1023px) {
  .footerPlayer {
    grid-template-columns: minmax(200px, 2fr) 3fr minmax(200px, 2fr);
  }

  .volumeProgressBar {
    width: 100px;
  }
}
@media (prefers-reduced-motion: reduce) {
  button,
  summary {
    transition: none;
  }
}
</style>
