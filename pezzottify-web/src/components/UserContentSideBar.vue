<template>
  <aside class="sidebar panel">
    <p v-if="loading">Loading...</p>
    <div v-else-if="albumIds">
      <h2>Liked Albums</h2>
      <div v-for="albumId in albumIds" :key="albumId">
        <AlbumCard :albumId="albumId" :showArtists="true" />
      </div>
    </div>
    <p v-else> {{ typeof (albumIds) }}</p>
  </aside>
</template>

<script setup>
import '@/assets/main.css';
import { watch, ref, onMounted } from 'vue';
import { useUserStore } from '@/store/user.js';
import AlbumCard from './common/AlbumCard.vue';

const userStore = useUserStore();

const albumIds = ref(null);
const loading = ref(true);

watch(() => userStore.isLoadingLikedAlbums,
  (isLoadingLikedAlbums) => {
    loading.value = isLoadingLikedAlbums;
  },
  { immediate: true }
);
watch(() => userStore.likedAlbumIds,
  (likedAlbums) => {
    console.log("UserContentSideBar likedAlbumIds: " + likedAlbums);
    console.log(likedAlbums);
    if (likedAlbums) {
      albumIds.value = likedAlbums;
    }
  },
  { immediate: true }
);

onMounted(() => {
  userStore.triggerAlbumsLoad();
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
</style>
