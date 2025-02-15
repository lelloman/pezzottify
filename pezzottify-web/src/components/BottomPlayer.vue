<template>
  <footer v-if="localCurrentTrack" class="footerPlayer">
    <div class="trackInfoRow">
      <img :src="imageUrl" alt="Image" class="trackImage" />
      <div class="trackNamesColumn">
        <h3 class="trackName"> {{ songName }}</h3>
        <p class="trackArtist"> {{ artistName }}</p>
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
        <TrackProgressBar class="rangeInput" :progress="combinedProgressPercent" @update:progress="updateProgress"
          @update:startDrag="startDragging" @update:stopDrag="seekTrack" />
        <span>{{ duration }}</span>
      </div>
    </div>
    <div class="extraControlsColumn">
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
import TrackProgressBar from './TrackProgressBar.vue';
import StopIcon from './icons/StopIcon.vue';

const ControlIconButton = {
  props: ["icon", "action"],
  setup(props) {
    const onClick = () => {
      props.action();
    }

    return () => h('div', { class: 'scalingIcon', onClick }, [
      h(props.icon, { class: 'lightControlFill' })
    ])
  },
};

const player = usePlayerStore();
const localCurrentTrack = ref(null);
const localProgressPercent = ref(0);

const combinedProgressPercent = computed(() => {
  return draggingPercent.value || localProgressPercent.value;
});
const draggingPercent = ref(null);

const { currentTrack, isPlaying, progressPercent, progressSec } = storeToRefs(player);

const songName = ref('');
const artistName = ref('');
const imageUrl = ref('');
const duration = ref('');

const currentTimeSec = ref(0);

const formatTime = (timeInSeconds) => {
  const hours = Math.floor(timeInSeconds / 3600);
  const minutes = Math.floor((timeInSeconds % 3600) / 60);
  const seconds = Math.floor(timeInSeconds % 60);

  const pad = (num) => String(num).padStart(2, '0');

  return `${pad(hours)}:${pad(minutes)}:${pad(seconds)}`;
};

const startDragging = () => {
  console.log("startDragging");
  draggingPercent.value = localProgressPercent.value;
}

const seekTrack = () => {
  if (draggingPercent.value) {
    const targetSeekPercent = draggingPercent.value;
    console.log("seekTrack target value: " + targetSeekPercent);
    player.seekToPercentage(targetSeekPercent)
    draggingPercent.value = null;
    console.log("stopDragging");
  }
};

const formattedTime = computed(() => formatTime(currentTimeSec.value));

function playPause() {
  console.log("BottomPlayer calling playPause() on PlayerStore.");
  player.playPause();
}

function updateProgress(event) {
  draggingPercent.value = event;
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

watch(progressPercent,
  (newProgressPercent) => {
    if (newProgressPercent) {
      //trackProgress.value = newProgressPercent;
      localProgressPercent.value = newProgressPercent;
    } else {
      //trackProgress.value = 0;
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
      artistName.value = newCurrentTrack.artist;
      imageUrl.value = newCurrentTrack.imageUrl;
      duration.value = formatDuration(newCurrentTrack.duration);
    } else {
      localCurrentTrack.value = null;
      songName.value = '';
      artistName.value = '';
      imageUrl.value = '';
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
  ;
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
  scale: 80%;
  transition: scale 0.3s ease;
  transform-origin: center;
  margin: 0 4px;
}

.scalingIcon:hover {
  scale: 100%;
  transition: scale 0.3s ease;
  cursor: pointer;
}

.scalingIcon:active {
  scale: 90%;
  transition: scale 0.3 ease;
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

.extraControlsColumn {
  height: 100%;
  align-content: center;
  flex: 1;
}

.rangeInput {
  max-width: 400px;
  width: 100%;
  flex: 1;
  margin: 0 12px;
}
</style>
