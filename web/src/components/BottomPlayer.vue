<template>
  <footer v-if="localCurrentTrack" class="footerPlayer">
    <div class="trackInfoRow">
      <MultiSourceImage :urls="imageUrls" alt="Image" class="trackImage scaleClickFeedback"
        @click.stop="handleClickOnAlbumCover" />
      <div class="trackNamesColumn">
        <TrackName :track="localCurrentTrack" :infiniteAnimation="true" />
        <LoadClickableArtistsNames :artistsIds="artists" />
      </div>
    </div>
    <div class="playerControlsColumn">
      <div class="playerControlsButtonsRow">
        <ControlIconButton :action="rewind10Sec" :icon="Rewind10Sec" />
        <ControlIconButton :action="skipPreviousTrack" :icon="SkipPrevious" />
        <ControlIconButton v-if="!isPlaying" :action="playPause" :icon="PlayIcon" :big="true" />
        <ControlIconButton v-if="isPlaying" :action="playPause" :icon="PauseIcon" :big="true" />
        <ControlIconButton :action="skipNextTrack" :icon="NextTrack" />
        <ControlIconButton :action="forward10Sec" :icon="Forward10Sec" />
      </div>
      <div class="progressControlsRow">
        <span>{{ formattedTime }}</span>
        <ProgressBar id="TrackProgressBar" class="trackProgressBar" :progress="combinedProgressPercent"
          @update:progress="updateTrackProgress" @update:startDrag="startDraggingTrackProgress"
          @update:stopDrag="seekTrack" />
        <span>{{ duration }}</span>
      </div>
    </div>
    <div class="extraControlsRow">
      <ControlIconButton v-if="isMuted" :action="volumeOn" :icon="VolumeOffIcon" />
      <ControlIconButton v-if="!isMuted" :action="volumeOff" :icon="VolumeOnIcon" />
      <ProgressBar class="volumeProgressBar" :progress="computedVolumePercent" @update:progress="updateVolumeProgress"
        @update:stratDrag="startDraggingVolumeProgress" @update:stopDrag="setVolume" />
      <ControlIconButton :action="stop" :icon="StopIcon" />
    </div>
  </footer>
</template>

<script setup>

import { computed, ref, watch, h } from 'vue';
import { usePlayerStore } from '@/store/player';
import { storeToRefs } from 'pinia';
import { formatDuration, chooseAlbumCoverImageUrl } from '@/utils';
import PlayIcon from './icons/PlayIcon.vue';
import PauseIcon from './icons/PauseIcon.vue';
import Forward10Sec from './icons/Forward10Sec.vue';
import Rewind10Sec from './icons/Rewind10Sec.vue';
import NextTrack from './icons/SkipNext.vue';
import SkipPrevious from './icons/SkipPrevious.vue';
import ProgressBar from '@/components/common/ProgressBar.vue';
import StopIcon from './icons/StopIcon.vue';
import VolumeOnIcon from './icons/VolumeOnIcon.vue';
import VolumeOffIcon from './icons/VolumeOffIcon.vue';
import MultiSourceImage from './common/MultiSourceImage.vue';
import LoadClickableArtistsNames from '@/components/common/LoadClickableArtistsNames.vue';
import { useRouter } from 'vue-router';
import TrackName from './common/TrackName.vue';
import { useStaticsStore } from '@/store/statics';

const ControlIconButton = {
  props: ["icon", "action", "big"],
  setup(props) {
    const onClick = () => {
      props.action();
    }

    const sizeClass = props.big ? 'bigIcon' : 'mediumIcon';

    return () => h('div', { class: 'lightControlFill scaleClickFeedback scalingIcon ' + sizeClass, onClick }, [
      h(props.icon)
    ])
  },
};

const router = useRouter();
const player = usePlayerStore();
const staticsStore = useStaticsStore();

const localCurrentTrack = ref(null);
const localProgressPercent = ref(0);

const combinedProgressPercent = computed(() => {
  return draggingTrackPercent.value || localProgressPercent.value;
});
const draggingTrackPercent = ref(null);

const volumePercent = ref(0.0);
const draggingVolumePercent = ref(null);
const isMuted = ref(false);

const computedVolumePercent = computed(() => {
  return draggingVolumePercent.value || (isMuted.value ? 0.0 : volumePercent.value);
})

const { currentTrackId, isPlaying, progressPercent, progressSec, volume, muted } = storeToRefs(player);

const songName = ref('');
const artists = ref([]);
const imageUrls = ref([]);
const duration = ref('');
const currentAlbumId = ref(null);

const currentTimeSec = ref(0);

let localCurrentTrackUnwatcher = null;
let albumDataUnwatcher = null;

const formatTime = (timeInSeconds) => {
  const hours = Math.floor(timeInSeconds / 3600);
  const minutes = Math.floor((timeInSeconds % 3600) / 60);
  const seconds = Math.floor(timeInSeconds % 60);

  const pad = (num) => String(num).padStart(2, '0');

  return `${pad(hours)}:${pad(minutes)}:${pad(seconds)}`;
};

const handleClickOnAlbumCover = () => {
  if (localCurrentTrack.value && localCurrentTrack.value.album_id) {
    router.push("/album/" + localCurrentTrack.value.album_id);
  }
}

const startDraggingTrackProgress = () => {
  console.log("startDragging");
  draggingTrackPercent.value = localProgressPercent.value;
}

const seekTrack = () => {
  if (draggingTrackPercent.value) {
    const targetSeekPercent = draggingTrackPercent.value;
    console.log("seekTrack target value: " + targetSeekPercent);
    player.seekToPercentage(targetSeekPercent)
    draggingTrackPercent.value = null;
    console.log("stopDragging");
  }
};

const formattedTime = computed(() => formatTime(currentTimeSec.value));

function playPause() {
  console.log("BottomPlayer calling playPause() on PlayerStore.");
  player.playPause();
}

function updateTrackProgress(event) {
  draggingTrackPercent.value = event;
}

function forward10Sec() {
  player.forward10Sec();
}

function rewind10Sec() {
  player.rewind10Sec();
}

function skipNextTrack() {
  player.skipNextTrack();
}

function skipPreviousTrack() {
  player.skipPreviousTrack();
}

function stop() {
  player.stop();
}

const startDraggingVolumeProgress = () => {
  draggingVolumePercent.value = volumePercent.value;
}

const updateVolumeProgress = (event) => {
  draggingVolumePercent.value = event;
}

const volumeOn = () => {
  player.setMuted(false);
}

const volumeOff = () => {
  player.setMuted(true);
}

const setVolume = () => {
  volumePercent.value = draggingVolumePercent.value;
  draggingVolumePercent.value = null;
  console.log("BottomPlayer setVolume " + volumePercent.value);
  player.setVolume(volumePercent.value);
  player.setMuted(false);
}

watch(progressPercent,
  (newProgressPercent) => {
    if (newProgressPercent) {
      localProgressPercent.value = newProgressPercent;
    } else {
      localProgressPercent.value = 0;
    }
  },
  { immediate: true }
);
watch(progressSec,
  (newProgressSec) => {
    if (newProgressSec) {
      currentTimeSec.value = newProgressSec;
    } else {
      currentTimeSec.value = 0;
    }
  },
  { immediate: true }
)
watch(currentTrackId,
  (newCurrentTrackId) => {
    if (localCurrentTrackUnwatcher) {
      localCurrentTrackUnwatcher();
      localCurrentTrackUnwatcher = null;
    }

    if (newCurrentTrackId) {
      localCurrentTrackUnwatcher = watch(staticsStore.getTrack(newCurrentTrackId),
        (newCurrentTrackRef) => {
          if (newCurrentTrackRef.item) {
            const newCurrentTrack = newCurrentTrackRef.item;

            localCurrentTrack.value = newCurrentTrack;
            songName.value = newCurrentTrack.name;
            artists.value = newCurrentTrack.artists_ids || [];
            duration.value = newCurrentTrack.duration ? formatDuration(newCurrentTrack.duration) : '';
            currentAlbumId.value = newCurrentTrack.album_id || null;
          }
        }, { immediate: true });
    } else {
      localCurrentTrack.value = null;
      songName.value = '';
      artists.value = [];
      imageUrls.value = [];
      duration.value = '';
      currentAlbumId.value = null;
    }
  },
  { immediate: true }
)

// Separate watcher for album cover image to avoid infinite loop
watch(currentAlbumId,
  (newAlbumId) => {
    if (albumDataUnwatcher) {
      albumDataUnwatcher();
      albumDataUnwatcher = null;
    }

    if (newAlbumId) {
      albumDataUnwatcher = watch(staticsStore.getAlbum(newAlbumId),
        (albumRef) => {
          if (albumRef && albumRef.item) {
            imageUrls.value = chooseAlbumCoverImageUrl(albumRef.item);
          }
        }, { immediate: true });
    } else {
      imageUrls.value = [];
    }
  },
  { immediate: true }
)
watch(isPlaying,
  (newIsPlaying) => {
    isPlaying.value = newIsPlaying;
    console.log("Bottom Player newIsPlaying: " + newIsPlaying);
  },
  { immediate: true }
);

watch(muted,
  (newMuted) => {
    isMuted.value = newMuted;
  },
  { immediate: true },
);
watch(volume,
  (newVolume) => {
    if (newVolume) {
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
  grid-template-columns: 30% 40% 30%;
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
  transition: transform var(--transition-base), box-shadow var(--transition-base);
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
  transition: transform var(--transition-fast), opacity var(--transition-fast);
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
  .playerControlsButtonsRow > :not(:nth-child(3)):not(:nth-child(2)):not(:nth-child(4)) {
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
    grid-template-columns: minmax(200px, 25%) 1fr minmax(200px, 25%);
  }

  .volumeProgressBar {
    width: 100px;
  }
}
</style>
