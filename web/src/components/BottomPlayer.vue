<template>
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
        <TrackName :track="displayTrack" :infiniteAnimation="true" />
        <LoadClickableArtistsNames :artistsIds="artists" />
      </div>
    </div>
    <div class="playerControlsColumn">
      <div class="playerControlsButtonsRow">
        <ControlIconButton :action="handleRewind10Sec" :icon="Rewind10Sec" />
        <ControlIconButton :action="handleSkipPrevious" :icon="SkipPrevious" />
        <ControlIconButton
          v-if="!isPlaying"
          :action="handlePlayPause"
          :icon="PlayIcon"
          :big="true"
        />
        <ControlIconButton
          v-if="isPlaying"
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
      <DeviceSelector />
      <ControlIconButton
        v-if="isMuted"
        :action="handleVolumeOn"
        :icon="VolumeOffIcon"
      />
      <ControlIconButton
        v-if="!isMuted"
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
      <ControlIconButton :action="handleStop" :icon="StopIcon" />
    </div>
  </footer>
</template>

<script setup>
import { computed, ref, watch, h } from "vue";
import { usePlayerStore } from "@/store/player";
import { storeToRefs } from "pinia";
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
import { useRemotePlaybackStore } from "@/store/remotePlayback";
import DeviceSelector from "./player/DeviceSelector.vue";

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
const player = usePlayerStore();
const staticsStore = useStaticsStore();
const remotePlayback = useRemotePlaybackStore();

// Track whether there's any playback to display
const hasPlayback = computed(() => {
  // Show player if we have local playback OR remote session
  return player.currentTrackId || remotePlayback.sessionExists;
});

// Use unified state from remotePlayback (handles local/remote transparently)
const isPlaying = computed(() => remotePlayback.isPlaying);
const currentProgressPercent = computed(() => remotePlayback.currentProgressPercent);
const currentPosition = computed(() => remotePlayback.currentPosition);

// Track info for display - combines local and remote
const displayTrack = ref(null);
const artists = ref([]);
const imageUrls = ref([]);
const duration = ref("");
const currentAlbumId = ref(null);

// Progress dragging state
const draggingTrackPercent = ref(null);
const combinedProgressPercent = computed(() => {
  return draggingTrackPercent.value ?? currentProgressPercent.value;
});

// Volume state
const volumePercent = ref(0.0);
const draggingVolumePercent = ref(null);
const isMuted = ref(false);

const computedVolumePercent = computed(() => {
  return (
    draggingVolumePercent.value ?? (isMuted.value ? 0.0 : volumePercent.value)
  );
});

// Get refs for local player state (for watching)
const { currentTrackId, volume, muted } = storeToRefs(player);

let trackDataUnwatcher = null;
let albumDataUnwatcher = null;

const formatTime = (timeInSeconds) => {
  const hours = Math.floor(timeInSeconds / 3600);
  const minutes = Math.floor((timeInSeconds % 3600) / 60);
  const seconds = Math.floor(timeInSeconds % 60);

  const pad = (num) => String(num).padStart(2, "0");

  return `${pad(hours)}:${pad(minutes)}:${pad(seconds)}`;
};

const formattedTime = computed(() => formatTime(currentPosition.value));

const handleClickOnAlbumCover = () => {
  if (displayTrack.value && displayTrack.value.album_id) {
    router.push("/album/" + displayTrack.value.album_id);
  }
};

// ============================================
// Playback controls - all route through remotePlayback
// ============================================

function handlePlayPause() {
  console.log("BottomPlayer calling playPause()");
  remotePlayback.playPause();
}

function handleSkipNext() {
  remotePlayback.skipNext();
}

function handleSkipPrevious() {
  remotePlayback.skipPrevious();
}

function handleForward10Sec() {
  remotePlayback.forward10Sec();
}

function handleRewind10Sec() {
  remotePlayback.rewind10Sec();
}

function handleStop() {
  remotePlayback.stop();
}

// Progress bar handling
const startDraggingTrackProgress = () => {
  console.log("startDragging");
  draggingTrackPercent.value = currentProgressPercent.value;
};

const updateTrackProgress = (event) => {
  draggingTrackPercent.value = event;
};

const handleSeek = () => {
  if (draggingTrackPercent.value !== null) {
    const targetSeekPercent = draggingTrackPercent.value;
    console.log("seekTrack target value: " + targetSeekPercent);
    remotePlayback.seekToPercentage(targetSeekPercent);
    draggingTrackPercent.value = null;
    console.log("stopDragging");
  }
};

// Volume handling
const startDraggingVolumeProgress = () => {
  draggingVolumePercent.value = volumePercent.value;
};

const updateVolumeProgress = (event) => {
  draggingVolumePercent.value = event;
};

const handleVolumeOn = () => {
  remotePlayback.setMuted(false);
};

const handleVolumeOff = () => {
  remotePlayback.setMuted(true);
};

const handleSetVolume = () => {
  if (draggingVolumePercent.value !== null) {
    volumePercent.value = draggingVolumePercent.value;
    draggingVolumePercent.value = null;
    console.log("BottomPlayer setVolume " + volumePercent.value);
    remotePlayback.setVolume(volumePercent.value);
    remotePlayback.setMuted(false);
  }
};

// ============================================
// Watch track info - handles both local and remote
// ============================================

// Watch local track changes
watch(
  currentTrackId,
  (newCurrentTrackId) => {
    if (trackDataUnwatcher) {
      trackDataUnwatcher();
      trackDataUnwatcher = null;
    }

    // Only watch local track if we're local output
    if (newCurrentTrackId && remotePlayback.isLocalOutput) {
      trackDataUnwatcher = watch(
        staticsStore.getTrack(newCurrentTrackId),
        (newCurrentTrackRef) => {
          if (newCurrentTrackRef.item) {
            const track = newCurrentTrackRef.item;
            displayTrack.value = track;
            artists.value = track.artists_ids || [];
            duration.value = track.duration
              ? formatDuration(track.duration)
              : "";
            currentAlbumId.value = track.album_id || null;
          }
        },
        { immediate: true },
      );
    }
  },
  { immediate: true },
);

// Watch remote track changes
watch(
  () => remotePlayback.currentTrack,
  (track) => {
    if (!remotePlayback.isLocalOutput && track) {
      displayTrack.value = {
        id: track.id,
        name: track.title,
        title: track.title,
        artists_ids: track.artist_id ? [track.artist_id] : [],
        album_id: track.album_id,
        duration: track.duration,
      };
      artists.value = track.artist_id ? [track.artist_id] : [];
      duration.value = track.duration ? formatDuration(track.duration) : "";
      currentAlbumId.value = track.album_id || null;
    }
  },
  { deep: true, immediate: true },
);

// Watch output mode changes - reset track display appropriately
watch(
  () => remotePlayback.isLocalOutput,
  (isLocal) => {
    if (isLocal) {
      // Switched to local - let the currentTrackId watcher handle it
      if (player.currentTrackId) {
        // Trigger track watcher
        const id = player.currentTrackId;
        if (trackDataUnwatcher) {
          trackDataUnwatcher();
        }
        trackDataUnwatcher = watch(
          staticsStore.getTrack(id),
          (ref) => {
            if (ref.item) {
              displayTrack.value = ref.item;
              artists.value = ref.item.artists_ids || [];
              duration.value = ref.item.duration
                ? formatDuration(ref.item.duration)
                : "";
              currentAlbumId.value = ref.item.album_id || null;
            }
          },
          { immediate: true },
        );
      }
    }
    // Remote case is handled by the remotePlayback.currentTrack watcher
  },
);

// Album cover image handling
watch(
  currentAlbumId,
  (newAlbumId) => {
    if (albumDataUnwatcher) {
      albumDataUnwatcher();
      albumDataUnwatcher = null;
    }

    if (newAlbumId) {
      albumDataUnwatcher = watch(
        staticsStore.getAlbum(newAlbumId),
        (albumRef) => {
          if (albumRef && albumRef.item) {
            imageUrls.value = chooseAlbumCoverImageUrl(albumRef.item);
          }
        },
        { immediate: true },
      );
    } else {
      imageUrls.value = [];
    }
  },
  { immediate: true },
);

// Volume state sync
watch(
  () => remotePlayback.currentVolume,
  (newVolume) => {
    if (newVolume !== undefined) {
      volumePercent.value = newVolume;
    }
  },
  { immediate: true },
);

watch(
  () => remotePlayback.currentMuted,
  (newMuted) => {
    isMuted.value = newMuted;
  },
  { immediate: true },
);

// Also watch local muted state when in local mode
watch(
  muted,
  (newMuted) => {
    if (remotePlayback.isLocalOutput) {
      isMuted.value = newMuted;
    }
  },
  { immediate: true },
);

watch(
  volume,
  (newVolume) => {
    if (remotePlayback.isLocalOutput && newVolume !== undefined) {
      volumePercent.value = newVolume;
    }
  },
  { immediate: true },
);
</script>

<style scoped>
@import "@/assets/icons.css";

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
  overflow: hidden;
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
