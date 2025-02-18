<template>
  <div v-if="panelVisible" class="currentPlayingSideBar panel">
    <div class="header">
      <h1>Currently Playing</h1>
    </div>
    <div class="trackRow" v-for="(track, index) in tracks" :class="{ currentlyPlayingRow: index == currentIndex }"
      :key="index" @click.stop="handleClick(index)">
      <MultiSourceImage class="trackImage" :urls="track.imageUrls" @click.stop="handleClickOnTrackImage(track)" />
      <div class="namesColumn">
        <p>{{ track.name }} </p>
        <ClickableArtistsNames :artistsIdsNames="track.artists" />
      </div>
      <p>{{ formatDuration(track.duration) }} </p>
    </div>
  </div>
</template>
<script setup>
import '@/assets/main.css';
import { watch, ref, computed } from 'vue';
import { usePlayerStore } from '@/store/player';
import { formatDuration } from '@/utils';
import MultiSourceImage from './common/MultiSourceImage.vue';
import ClickableArtistsNames from './common/ClickableArtistsNames.vue';
import { useRouter } from 'vue-router';

const panelVisible = computed(() => tracks.value.length);
const tracks = ref([]);
const currentIndex = ref(null);

const router = useRouter();
const player = usePlayerStore();

const handleClick = (index) => {
  player.loadTrackIndex(index);
}

const handleClickOnTrackImage = (track) => {
  router.push("/album/" + track.albumId);
}

watch(
  () => player.currentTrackIndex,
  (index) => {
    currentIndex.value = index;
  },
  { immediate: true }
)

watch(
  () => player.playlist,
  (playlist) => {
    if (playlist && playlist.tracks) {
      tracks.value = playlist.tracks.map((track) => {
        return track;
      });
    } else {
      tracks.value = [];
    }
  },
  { immediate: true }
);

</script>

<style scoped>
.currentPlayingSideBar {
  overflow-x: hidden;
  overflow-y: auto;
  min-width: 200px;
  width: 20%;
  max-width: 600px;
  box-sizing: border-box;
  margin-left: 8px;
  margin-bottom: 16px;
  margin-right: 16px;
}

.header {
  padding: 8px 16px;
}

.trackRow {
  display: flex;
  flex-direction: row;
  cursor: pointer;
  padding: 8px 16px;
  align-content: center;
  align-items: center;
}

.trackRow:hover {
  background-color: var(--highlighted-panel-color);
}

.trackImage {
  width: 40px;
  height: 40px;
  transition: scale 0.3s ease;
  cursor: pointer;
}

.trackImage:hover {
  scale: 1.1;
  transition: 0.3s ease;
}

.namesColumn {
  flex: 1;
  display: flex;
  flex-direction: column;
  padding: 0 8px;
}

.currentlyPlayingRow {
  color: var(--accent-color);
  text-decoration: bold;
}
</style>
