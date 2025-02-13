<template>
  <footer v-if="currentTrack" class="footerPlayer">
    <audio ref="audioPlayer" :src="currentTrack.url" @timeupdate="updateProgress"></audio>
    <div>Now Playing: Song Title</div>
    <div class="flex items-center space-x-4">
      <button @click="playPause">{{ isPlaying ? 'Pause' : 'Play' }}</button>
      <button @click="stop">Stop</button>
      <input type="range" v-model="progress" max="100" />
      <span>{{ formattedTime }}</span> <button class="p-2 bg-gray-700 rounded">Prev</button>
      <button class="p-2 bg-gray-700 rounded">Play/Pause</button>
      <button class="p-2 bg-gray-700 rounded">Next</button>
    </div>
  </footer>
</template>

<script setup>

import { usePlayerStore } from '@/store/player';

const { playlist, currentTrack, isPlaying, setTrack, playPause } = usePlayerStore();

</script>

<style scoped>
.footerPlayer {
  height: 120px;
}
</style>
