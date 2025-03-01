<template>
  <div v-if="panelVisible" class="panel containero">
    <div class="header">
      <h1>Currently Playing</h1>
    </div>
    <div class="trackRowsContainer">
      <div class="trackRow" v-for="(track, index) in tracks" :class="{ currentlyPlayingRow: index == currentIndex }"
        :key="index" @click.stop="handleClick(index)" @contextmenu.prevent="openContextMenu($event, track)">
        <MultiSourceImage class="trackImage scaleClickFeedback" :urls="track.imageUrls"
          @click.stop="handleClickOnTrackImage(track)" />
        <div class="namesColumn">
          <TrackName :track="track" :hoverAnimation="true" />
          <ClickableArtistsNames :artistsIdsNames="track.artists" />
        </div>
        <p>{{ formatDuration(track.duration) }} </p>
      </div>
    </div>

    <TrackContextMenu ref="trackContextMenuRef" />
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
import TrackName from './common/TrackName.vue';
import TrackContextMenu from '@/components/common/contextmenu/TrackContextMenu.vue';

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

const trackContextMenuRef = ref(null);

const openContextMenu = (event, track) => {
  trackContextMenuRef.value.openMenu(event, track);
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
.containero {
  display: flex;
  flex-direction: column;
}

.header {
  padding: 8px 16px;
}

.trackRowsContainer {
  display: flex;
  width: 100%;
  flex-direction: column;
  overflow-y: auto;
  flex: 1;
}

.trackRow {
  display: flex;
  width: 100%;
  flex: 1;
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
}

.namesColumn {
  flex: 1;
  width: 0;
  display: flex;
  flex-direction: column;
  padding: 0 8px;
}

.currentlyPlayingRow {
  color: var(--accent-color);
  text-decoration: bold;
}
</style>
