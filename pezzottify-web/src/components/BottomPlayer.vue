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
      <div class="playerControlButtonsRow">
        <button class="">-10s</button>
        <button class="">Next</button>
        <button @click="playPause">{{ isPlaying ? 'Pause' : 'Play' }}</button>
        <button class="">Prev</button>
        <button class="">+10s</button>
      </div>
      <div class="progressControlsRow">
        <span>{{ formattedTime }}</span>
        <input class="rangeInput" type="range" :value="combinedProgressPercent" max="100" @mousedown="startDragging"
          @input="updateProgress" @change="seekTrack" />
        <span>{{ duration }}</span>
      </div>
    </div>
    <div class="extraControlsColumn">
      <button @click="stop">Stop</button>
    </div>
  </footer>
</template>

<script setup>

import { computed, ref, watch } from 'vue';
import { usePlayerStore } from '@/store/player';
import { storeToRefs } from 'pinia';
import { formatDuration } from '@/utils';

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

const seekTrack = (event) => {
  const targetSeekPercent = parseFloat(event.target.value);
  console.log("seekTrack target value: " + targetSeekPercent);
  player.seekToPercentage(targetSeekPercent / 100.0)
  draggingPercent.value = null;
  console.log("stopDragging");
};

const formattedTime = computed(() => formatTime(currentTimeSec.value));

function playPause() {
  console.log("BottomPlayer calling playPause() on PlayerStore.");
  player.playPause();
}

function updateProgress(event) {
  draggingPercent.value = event.target.value;
}

watch(progressPercent,
  (newProgressPercent) => {
    localProgressPercent.value = newProgressPercent * 100;
  },
  { immediate: true }
);
watch(progressSec,
  (newProgressSec) => {
    if (newProgressSec) {
      currentTimeSec.value = newProgressSec;
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
.footerPlayer {
  min-width: 800px;
  height: 120px;
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
}

.trackImage {
  width: 80px;
  height: 80px;
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
  align-content: center;
  flex: 1;
}

.progressControlsRow {
  display: flex;
  flex-direction: row;
  align-items: center;
  align-content: center;
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
}
</style>
