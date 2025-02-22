<template>
  <aside class="sidebar panel">
    <div class="tabSelectorsContainer">
      <div @click.stop="selectedTab = 'albums'"
        :class="{ 'tabSelector': true, 'selectedTab': selectedTab === 'albums' }">
        <h3>Albums</h3>
      </div>
      <div @click.stop="selectedTab = 'artists'"
        :class="{ 'tabSelector': true, 'selectedTab': selectedTab === 'artists' }">
        <h3>Artists</h3>
      </div>
    </div>
    <div v-if="selectedTab == 'albums'">
      <div v-if="isLoadingAlbums">Loading...</div>
      <div ref="albumsRef" v-for="albumId in albumIds" :key="albumId" :mounted="userStore.triggerAlbumsLoad()">
        <AlbumCard :albumId="albumId" :showArtists="true" />
      </div>
    </div>
    <div v-else-if="selectedTab == 'artists'">
      <div v-if="isLoadingArtists">Loading...</div>
      <div ref="artistsRef" v-for="artistId in artistsIds" :key="artistId" :mounted="userStore.triggerArtistsLoad()">
        <LoadArtistListItem :artistId="artistId" />
      </div>
    </div>
  </aside>
</template>

<script setup>
import '@/assets/main.css';
import { watch, ref, onMounted } from 'vue';
import { useUserStore } from '@/store/user.js';
import AlbumCard from './common/AlbumCard.vue';
import LoadArtistListItem from './common/LoadArtistListItem.vue';

const userStore = useUserStore();

const albumIds = ref(null);
const artistsIds = ref(null);
const isLoadingArtists = ref(true);
const isLoadingAlbums = ref(true);

const selectedTab = ref('albums');

watch(selectedTab, (tab) => {
  if (tab === 'albums') {
    userStore.triggerAlbumsLoad();
  } else if (tab === 'artists') {
    userStore.triggerArtistsLoad();
  }
}, {
  immediate: true
});

watch(() => userStore.isLoadingLikedAlbums,
  (isLoading) => {
    isLoadingAlbums.value = isLoading;
  },
  { immediate: true }
);
watch(() => userStore.isLoadingLikedArtists,
  (isLoading) => {
    isLoadingArtists.value = isLoading;
  },
  { immediate: true }
);
watch(() => userStore.likedAlbumIds,
  (likedAlbums) => {
    if (likedAlbums) {
      albumIds.value = likedAlbums;
    }
  },
  { immediate: true }
);
watch(() => userStore.likedArtistsIds,
  (likedArtists) => {
    if (likedArtists) {
      artistsIds.value = likedArtists;
    }
  },
  { immediate: true }
);

</script>

<style scoped>
.sidebar {
  min-width: 200px;
  width: 20%;
  max-width: 600px;
  margin-left: 16px;
  margin-bottom: 16px;
  margin-right: 8px;
  overflow-y: auto;
}

.tabSelectorsContainer {
  display: flex;
  justify-content: space-around;
  margin: 16px 0;
}

.tabSelector {
  cursor: pointer;
  padding: 8px 16px;
  border-radius: 8px;
  transition: scale 0.3s ease;
}

.tabSelector>h3 {
  color: white;
  font-weight: bold;
}

.tabSelector:hover {
  background-color: var(--highlighted-panel-color);
  transition: scale 0.3s ease;
  scale: 1.1;
}

.tabSelector:active {
  transition: scale 0.3s ease;
  scale: 0.9;
}

.selectedTab {
  background-color: var(--accent-color) !important;
}
</style>
