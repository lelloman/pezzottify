<template>
  <div class="mainContainer">
    <!-- Top Bar -->
    <TopBar @search="handleSearch" />
    <div class="centralPanel">
      <!-- Sidebar -->
      <SideBar :items="sidebarItems" @select-item="handleSelect" />
      <!-- Main Content -->
      <MainContent :search-query="searchQuery" />
    </div>
    <!-- Bottom Player -->
    <BottomPlayer />
  </div>
</template>

<script setup>
import { ref } from 'vue';
import SideBar from '../components/SideBar.vue';
import TopBar from '../components/TopBar.vue';
import MainContent from '../components/MainContent.vue';
import BottomPlayer from '../components/BottomPlayer.vue';

const sidebarItems = ref([
  { id: 1, name: 'Home', type: 'link' },
  { id: 2, name: 'Albums', type: 'section', items: ['Album 1', 'Album 2', 'Album 3'] },
  { id: 3, name: 'Playlists', type: 'section', items: ['Playlist 1', 'Playlist 2'] }
]);

const searchQuery = ref('');

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
}
</style>