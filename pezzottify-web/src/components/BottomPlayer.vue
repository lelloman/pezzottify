<template>
  <footer v-if="localCurrentTrack" class="footerPlayer">
    <div class="trackInfoRow">
      <MultiSourceImage :urls="imageUrls" alt="Image" class="trackImage scaleClickFeedback"
        @click.stop="handleClickOnAlbumCover" />
      <div class="trackNamesColumn">
        <TrackName :track="localCurrentTrack" />
        <ClickableArtistsNames :artistsIdsNames="artists" />
      </div>
    </div>
    <div class="playerControlsColumn">
      <div class="playerControlsButtonsRow">
        <ControlIconButton :action="rewind10Sec" :icon="Rewind10Sec" />
        <ControlIconButton :action="skipPreviousTrack" :icon="SkipPrevious" />
        <ControlIconButton v-if="!isPlaying" :action="playPause" :icon="PlayIcon" />
        <ControlIconButton v-if="isPlaying" :action="playPause" :icon="PauseIcon" />
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
import { formatDuration } from '@/utils';
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
import ClickableArtistsNames from './common/ClickableArtistsNames.vue';
import { useRouter } from 'vue-router';
import TrackName from './common/TrackName.vue';

const ControlIconButton = {
  props: ["icon", "action"],
  setup(props) {
    const onClick = () => {
      props.action();
    }

    return () => h('div', { class: 'scalingIcon scaleClickFeedback', onClick }, [
      h(props.icon, { class: 'lightControlFill' })
    ])
  },
};

const router = useRouter();
const player = usePlayerStore();
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

const { currentTrack, isPlaying, progressPercent, progressSec, volume, muted } = storeToRefs(player);

const songName = ref('');
const artists = ref([]);
const imageUrls = ref([]);
const duration = ref('');

const currentTimeSec = ref(0);

const formatTime = (timeInSeconds) => {
  const hours = Math.floor(timeInSeconds / 3600);
  const minutes = Math.floor((timeInSeconds % 3600) / 60);
  const seconds = Math.floor(timeInSeconds % 60);

  const pad = (num) => String(num).padStart(2, '0');

  return `${pad(hours)}:${pad(minutes)}:${pad(seconds)}`;
};

const handleClickOnAlbumCover = () => {
  console.log(currentTrack.value);
  router.push("/album/" + currentTrack.value.albumId);
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
watch(currentTrack,
  (newCurrentTrack) => {
    console.log("BottomPlayer newCurrentTrack:");
    console.log(newCurrentTrack);
    if (newCurrentTrack) {
      localCurrentTrack.value = newCurrentTrack;
      songName.value = newCurrentTrack.name;
      artists.value = newCurrentTrack.artists;
      imageUrls.value = newCurrentTrack.imageUrls;
      duration.value = formatDuration(newCurrentTrack.duration);
    } else {
      localCurrentTrack.value = null;
      songName.value = '';
      artists.value = [];
      imageUrls.value = [];
      duration.value = '';
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

.footerPlayer {
  min-width: 800px;
  height: 100px;
  display: flex;
  flex-direction: row;
  overflow: hidden;
}

.trackInfoRow {
  padding: 16px;
  text-align: left;
  flex: 1;
  display: flex;
  flex-direction: row;
  align-items: center;
}

.trackImage {
  width: 56px;
  height: 56px;
  border-radius: 4px;
  cursor: pointer;
}

.trackNamesColumn {
  flex: 1;
  flex-direction: column;
  padding: 16px;
}

.trackName {
  margin: 0;
  font-size: 16px;
  font-weight: bold;
}

.trackArtist {
  margin: 0;
  font-size: 14px;
  color: #666;
}

.playerControlsColumn {
  height: 100%;
  flex: 1;
  display: flex;
  flex-direction: column;
}

.scalingIcon {
  transform-origin: center;
  margin: 0 4px;
}

.playerControlsButtonsRow {
  display: flex;
  flex-direction: row;
  justify-content: center;
  align-items: center;
}

.progressControlsRow {
  flex: 1;
  display: flex;
  flex-direction: row;
  align-items: center;
  justify-content: center;
  margin-bottom: 16px;
}

.trackProgressBar {
  max-width: 400px;
  width: 100%;
  flex: 1;
  margin: 0 12px;
}

.extraControlsRow {
  height: 100%;
  align-content: center;
  flex: 1;
  display: flex;
  flex-direction: row;
  justify-content: center;
  align-items: center;
}

.volumeProgressBar {
  max-width: 150px;
  margin: 0 12px;
}
</style>
