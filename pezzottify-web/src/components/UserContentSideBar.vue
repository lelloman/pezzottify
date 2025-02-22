<template>
  <aside class="sidebar panel">
    <p v-if="loading">Loading...</p>
    <div v-else-if="albumIds && artistsIds">
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
        <div v-for="albumId in albumIds" :key="albumId">
          <AlbumCard :albumId="albumId" :showArtists="true" />
        </div>
      </div>
      <div v-else-if="selectedTab == 'artists'">
        <div v-for="artistId in artistsIds" :key="artistId">
          <LoadArtistListItem :artistId="artistId" />
        </div>
      </div>
    </div>
    <p v-else> {{ albumIds }} <br><br> {{ artistsIds }}</p>

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
const loading = ref(true);

const selectedTab = ref('albums');

watch([() => userStore.isLoadingLikedAlbums, userStore.isLoadingLikedArtists],
  ([isLoadingLikedAlbums, isLoadingLikedArtists], [oldIsLoadingLikedAlbums, oldIsLoadingLikedArtists]) => {
    loading.value = isLoadingLikedAlbums || isLoadingLikedArtists;
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

onMounted(() => {
  userStore.triggerAlbumsLoad();
  userStore.triggerArtistsLoad();
});
</script>

<style scoped>
.sidebar {
  min-width: 200px;
  width: 20%;
  max-width: 600px;
  margin-left: 16px;
  margin-bottom: 16px;
  margin-right: 8px;
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
