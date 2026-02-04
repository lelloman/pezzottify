<template>
  <div
    v-if="showRemoteBanner"
    class="remoteBanner"
  >
    <span v-if="playback.mode === 'remote'" class="remoteBannerText">
      Playing on {{ sessionStore.audioDeviceName || 'another device' }}
    </span>
    <span v-else class="remoteBannerText">
      Music playing on {{ sessionStore.audioDeviceName || 'another device' }}
    </span>
    <button
      v-if="playback.mode !== 'remote'"
      class="remoteBannerAction"
      @click="handleEnterRemoteMode"
    >
      Control it
    </button>
    <button
      v-if="playback.mode !== 'remote'"
      class="remoteBannerDismiss"
      @click="dismissBanner"
    >
      Dismiss
    </button>
  </div>
  <footer v-if="hasPlayback" class="footerPlayer">
    <div class="trackInfoRow">
      <MultiSourceImage
        :urls="imageUrls"
        :lazy="false"
        alt="Image"
        class="trackImage scaleClickFeedback"
        @click.stop="handleClickOnAlbumCover"
      />
      <div class="trackNamesColumn">
        <TrackName v-if="displayTrack" :track="displayTrack" :infiniteAnimation="true" />
        <LoadClickableArtistsNames v-if="artists.length > 0" :artistsIds="artists" />
        <span v-else-if="artistName" class="artistName">{{ artistName }}</span>
      </div>
    </div>
    <div class="playerControlsColumn">
      <div class="playerControlsButtonsRow">
        <ControlIconButton :action="handleRewind10Sec" :icon="Rewind10Sec" />
        <ControlIconButton :action="handleSkipPrevious" :icon="SkipPrevious" />
        <ControlIconButton
          v-if="!playback.isPlaying"
          :action="handlePlayPause"
          :icon="PlayIcon"
          :big="true"
        />
        <ControlIconButton
          v-if="playback.isPlaying"
          :action="handlePlayPause"
          :icon="PauseIcon"
          :big="true"
        />
        <ControlIconButton :action="handleSkipNext" :icon="NextTrack" />
        <ControlIconButton :action="handleForward10Sec" :icon="Forward10Sec" />
      </div>
      <div class="progressControlsRow">
        <span>{{ formattedTime }}</span>
        <ProgressBar
          id="TrackProgressBar"
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
        :action="handleVolumeOn"
        :icon="VolumeOffIcon"
      />
      <ControlIconButton
        v-if="!playback.muted"
        :action="handleVolumeOff"
        :icon="VolumeOnIcon"
      />
      <ProgressBar
        class="volumeProgressBar"
        :progress="computedVolumePercent"
        @update:progress="updateVolumeProgress"
        @update:stratDrag="startDraggingVolumeProgress"
        @update:stopDrag="handleSetVolume"
      />
      <DeviceSelector />
      <ControlIconButton v-if="playback.mode === 'local'" :action="handleStop" :icon="StopIcon" />
    </div>
  </footer>
</template>

<script setup>
import { computed, ref, watch, h } from "vue";
import { usePlaybackStore } from "@/store/playback";
import { formatDuration, chooseAlbumCoverImageUrl } from "@/utils";
import PlayIcon from "./icons/PlayIcon.vue";
import PauseIcon from "./icons/PauseIcon.vue";
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
import { useRouter } from "vue-router";
import TrackName from "./common/TrackName.vue";
import { useStaticsStore } from "@/store/statics";
import { usePlaybackSessionStore } from "@/store/playbackSession";
import DeviceSelector from "./DeviceSelector.vue";

const ControlIconButton = {
  props: ["icon", "action", "big"],
  setup(props) {
    const onClick = () => {
      props.action();
    };

    const sizeClass = props.big ? "bigIcon" : "mediumIcon";

    return () =>
      h(
        "div",
        {
          class: "lightControlFill scaleClickFeedback scalingIcon " + sizeClass,
          onClick,
        },
        [h(props.icon)],
      );
  },
};

const router = useRouter();
const playback = usePlaybackStore();
const staticsStore = useStaticsStore();
const sessionStore = usePlaybackSessionStore();

// Track whether there's any playback to display
const hasPlayback = computed(() => {
  return playback.currentTrackId;
});

// Remote mode banner state
const bannerDismissed = ref(false);

const showRemoteBanner = computed(() => {
  if (playback.mode === "remote") return true;

  // Show "ask" banner when there's a remote session we could control
  if (
    !bannerDismissed.value &&
    sessionStore.sessionExists &&
    sessionStore.audioDeviceId !== sessionStore.myDeviceId &&
    sessionStore.role === "idle" &&
    sessionStore.getRemoteControlPreference() === "ask"
  ) {
    return true;
  }

  return false;
});

function handleEnterRemoteMode() {
  sessionStore.enterRemoteMode();
}

function dismissBanner() {
  bannerDismissed.value = true;
}

// Reset banner dismissal when session state changes
watch(
  () => sessionStore.sessionExists,
  () => {
    bannerDismissed.value = false;
  },
);

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

  return `${pad(hours)}:${pad(minutes)}:${pad(seconds)}`;
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
      duration.value = track.duration ? formatDuration(track.duration) : "";
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
              ? formatDuration(localTrack.duration)
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
                { immediate: true }
              );
            }
          }
        },
        { immediate: true }
      );
    }
  },
  { immediate: true, deep: true }
);
</script>

<style scoped>
@import "@/assets/icons.css";

/* ============================================
   Remote Banner
   ============================================ */

.remoteBanner {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--spacing-3);
  padding: var(--spacing-1) var(--spacing-4);
  background-color: var(--spotify-green);
  color: var(--bg-base);
  font-size: var(--text-sm);
}

.remoteBannerText {
  font-weight: var(--font-medium);
}

.remoteBannerAction {
  padding: 2px var(--spacing-3);
  font-size: var(--text-xs);
  font-weight: var(--font-semibold);
  background-color: var(--bg-base);
  color: var(--spotify-green);
  border: none;
  border-radius: var(--radius-full);
  cursor: pointer;
  transition: opacity var(--transition-fast);
}

.remoteBannerAction:hover {
  opacity: 0.9;
}

.remoteBannerDismiss {
  padding: 2px var(--spacing-2);
  font-size: var(--text-xs);
  background: transparent;
  color: var(--bg-base);
  border: 1px solid var(--bg-base);
  border-radius: var(--radius-full);
  cursor: pointer;
  opacity: 0.7;
  transition: opacity var(--transition-fast);
}

.remoteBannerDismiss:hover {
  opacity: 1;
}

/* ============================================
   Footer Player - Desktop Layout
   ============================================ */

.footerPlayer {
  height: var(--player-height-desktop);
  display: grid;
  grid-template-columns: 3fr 4fr 3fr;
  gap: var(--spacing-4);
  padding: 0 var(--spacing-4);
  align-items: center;
  background-color: var(--bg-base);
  border-top: 1px solid var(--border-default);
}

/* ============================================
   Track Info Section (Left 30%)
   ============================================ */

.trackInfoRow {
  display: flex;
  flex-direction: row;
  align-items: center;
  gap: var(--spacing-3);
  min-width: 0;
  text-align: left;
}

.trackImage {
  width: 56px;
  height: 56px;
  min-width: 56px;
  border-radius: var(--radius-md);
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
  gap: var(--spacing-1);
}

.trackName {
  margin: 0;
  font-size: var(--text-base);
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
  gap: var(--spacing-2);
  min-width: 0;
}

.playerControlsButtonsRow {
  display: flex;
  flex-direction: row;
  justify-content: center;
  align-items: center;
  gap: var(--spacing-1);
}

.scalingIcon {
  transform-origin: center;
  transition:
    transform var(--transition-fast),
    opacity var(--transition-fast);
}

.scalingIcon:hover {
  transform: scale(1.06);
}

.scalingIcon:active {
  transform: scale(0.96);
}

.progressControlsRow {
  display: flex;
  flex-direction: row;
  align-items: center;
  gap: var(--spacing-3);
  min-width: 0;
}

.progressControlsRow span {
  font-size: var(--text-xs);
  font-weight: var(--font-normal);
  color: var(--text-subdued);
  min-width: 56px;
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
  gap: var(--spacing-2);
  min-width: 0;
}

.volumeProgressBar {
  width: 120px;
  max-width: 120px;
}

/* ============================================
   Icon Button Styling
   ============================================ */

.lightControlFill {
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  color: var(--text-base);
  border-radius: var(--radius-full);
  transition: all var(--transition-fast);
}

.lightControlFill:hover {
  color: var(--text-bright);
}

.lightControlFill:focus-visible {
  outline: 2px solid var(--spotify-green);
  outline-offset: 2px;
}

.mediumIcon {
  width: 32px;
  height: 32px;
  padding: var(--spacing-1);
}

.bigIcon {
  width: 48px;
  height: 48px;
  padding: var(--spacing-2);
  background-color: var(--spotify-green);
  color: var(--bg-base);
}

.bigIcon:hover {
  background-color: var(--spotify-green-hover);
  transform: scale(1.06);
  color: var(--bg-base);
}

.bigIcon:active {
  background-color: var(--spotify-green-active);
  transform: scale(0.96);
}

/* ============================================
   Mobile Layout (< 768px)
   ============================================ */

@media (max-width: 767px) {
  .footerPlayer {
    height: var(--player-height-mobile);
    grid-template-columns: 1fr auto auto;
    grid-template-rows: 4px 1fr;
    gap: var(--spacing-2);
    padding: 0 var(--spacing-3);
  }

  .trackInfoRow {
    grid-column: 1;
    grid-row: 2;
    gap: var(--spacing-2);
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
    grid-column: 2;
    grid-row: 2;
    gap: 0;
  }

  .playerControlsButtonsRow {
    gap: var(--spacing-1);
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
    grid-column: 3;
    grid-row: 2;
    gap: var(--spacing-1);
  }

  .volumeProgressBar {
    display: none;
  }

  .mediumIcon {
    width: 28px;
    height: 28px;
  }

  .bigIcon {
    width: 40px;
    height: 40px;
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
</style>
