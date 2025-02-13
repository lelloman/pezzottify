<template>
  <footer v-if="localCurrentTrack" class="footerPlayer">
    <div>Now Playing: Song Title</div>
    <div class="flex items-center space-x-4">
      <button @click="playPause">{{ isPlaying ? 'Pause' : 'Play' }}</button>
      <button @click="stop">Stop</button>
      <input type="range" v-model="localProgressPercent" max="100" @input="updateProgress" @change="seekTrack" />
      <span>{{ formattedTime }}</span> <button class="p-2 bg-gray-700 rounded">Prev</button>
      <button class="p-2 bg-gray-700 rounded">Play/Pause</button>
      <button class="p-2 bg-gray-700 rounded">Next</button>
    </div>
  </footer>
</template>

<script setup>

import { onMounted, computed, ref, watch } from 'vue';
import { usePlayerStore } from '@/store/player';
import { storeToRefs } from 'pinia';

const player = usePlayerStore();
const localCurrentTrack = ref(null);
const localProgressPercent = ref(0);

const { currentTrack, isPlaying, progressPercent, progressSec } = storeToRefs(player);

const currentTimeSec = ref(0);

const formatTime = (timeInSeconds) => {
  const hours = Math.floor(timeInSeconds / 3600);
  const minutes = Math.floor((timeInSeconds % 3600) / 60);
  const seconds = Math.floor(timeInSeconds % 60);

  const pad = (num) => String(num).padStart(2, '0');

  return `${pad(hours)}:${pad(minutes)}:${pad(seconds)}`;
};

const seekTrack = (event) => {
  const targetSeekPercent = parseFloat(event.target.value);
  console.log("seekTrack target value: " + targetSeekPercent);
  player.seekToPercentage(targetSeekPercent / 100.0)
};

const formattedTime = computed(() => formatTime(currentTimeSec.value));

function playPause() {
  console.log("BottomPlayer calling playPause() on PlayerStore.");
  player.playPause();
}

function updateProgress(event) {
  localProgressPercent.value = event.target.value;
  console.log("updateProgress() " + localProgressPercent.value);
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
    console.log("BottomPlayer newCurrentTrack " + newCurrentTrack);
    if (newCurrentTrack) {
      localCurrentTrack.value = newCurrentTrack;
    } else {
      localCurrentTrack.value = null;
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
  height: 120px;
}
</style>
