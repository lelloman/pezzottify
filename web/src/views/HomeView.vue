<template>
  <div class="mainContainer">
    <TopBar @search="handleSearch" :initialQuery="searchQuery" />
    <div class="centralPanel">
      <UserContentSideBar :items="sidebarItems" @select-item="handleSelect" />
      <MainContent :search-query="searchQuery" />
      <CurrentPlayingSideBar />
    </div>
    <BottomPlayer />
  </div>
</template>


<script setup>
import { ref, watch } from 'vue';
import TopBar from '@/components/TopBar.vue';
import MainContent from '@/components/content/MainContent.vue';
import BottomPlayer from '@/components/BottomPlayer.vue';
import { useRoute } from 'vue-router';
import UserContentSideBar from '@/components/UserContentSideBar.vue';
import CurrentPlayingSideBar from '@/components/CurrentPlayingSideBar.vue';

const sidebarItems = ref([
  { id: 1, name: 'Home', type: 'link' },
  { id: 2, name: 'Albums', type: 'section', items: ['Album 1', 'Album 2', 'Album 3'] },
  { id: 3, name: 'Playlists', type: 'section', items: ['Playlist 1', 'Playlist 2'] }
]);

const route = useRoute();
const searchQuery = ref(decodeURIComponent(route.params.query || ''));

// Watch for changes in the route's query parameter
watch(
  () => route.params.query,
  (newQuery) => {
    searchQuery.value = decodeURIComponent(newQuery || '');
  },
  { immediate: true }
);

function handleSearch(query) {
  searchQuery.value = query;
}

function handleSelect(item) {
  console.log('Selected:', item);
}
</script>

<style>
body {
  @apply bg-gray-900 text-gray-100;
}

.mainContainer {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
}

.centralPanel {
  flex: 1;
  display: flex;
  flex-direction: row;
  height: 100%;
  overflow: hidden;
  text-align: left !important;
}
</style>
