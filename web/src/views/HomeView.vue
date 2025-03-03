<template>
  <div class="mainContainer">
    <div v-if="isLoading" class="loading-container">
      <div class="loader"></div>
      <p>Loading your content...</p>
    </div>
    <template v-else>
      <TopBar @search="handleSearch" :initialQuery="searchQuery" />
      <div class="centralPanel">
        <UserContentSideBar @select-item="handleSelect" class="sideBar userContentSideBar" />
        <MainContent :search-query="searchQuery" />
        <CurrentlyPlayingSideBar class="sideBar currentlyPlayingSideBar" />
      </div>
      <BottomPlayer />
    </template>
  </div>
</template>


<script setup>
import { ref, watch, onMounted } from 'vue';
import TopBar from '@/components/TopBar.vue';
import MainContent from '@/components/content/MainContent.vue';
import BottomPlayer from '@/components/BottomPlayer.vue';
import { useRoute } from 'vue-router';
import UserContentSideBar from '@/components/UserContentSideBar.vue';
import CurrentlyPlayingSideBar from '@/components/CurrentlyPlayingSideBar.vue';
import { useUserStore } from '@/store/user';

// Access the user store
const userStore = useUserStore();
const isLoading = ref(true);

// Initialize the store when the component is mounted
onMounted(async () => {
  try {
    await userStore.initialize();
  } catch (error) {
    console.error('Failed to initialize user data:', error);
  } finally {
    isLoading.value = false;
  }
});

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

.sideBar {
  min-width: 200px;
  max-width: 600px;
  width: 20%;
}

.userContentSideBar {
  display: flex;
  flex-direction: column;
  margin-left: 16px;
  margin-right: 8px;
}

.currentlyPlayingSideBar {
  overflow-y: auto;
  overflow-x: hidden;
  box-sizing: border-box;
  margin-right: 16px;
}

.loading-container {
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: center;
  height: 100vh;
  width: 100%;
}

.loader {
  border: 5px solid rgba(255, 255, 255, 0.3);
  border-radius: 50%;
  border-top: 5px solid #1DB954;
  /* Spotify green color */
  width: 50px;
  height: 50px;
  animation: spin 1s linear infinite;
  margin-bottom: 20px;
}

@keyframes spin {
  0% {
    transform: rotate(0deg);
  }

  100% {
    transform: rotate(360deg);
  }
}
</style>
