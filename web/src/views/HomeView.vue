<template>
  <div class="mainContainer">
    <div v-if="isLoading" class="loading-container">
      <div class="loader"></div>
      <p>Loading your content...</p>
    </div>
    <template v-else>
      <TopBar @search="handleSearch" :initialQuery="searchQuery" />
      <div class="centralPanel">
        <UserContentSideBar
          @select-item="handleSelect"
          class="sideBar userContentSideBar"
        />
        <MainContent :search-query="searchQuery" />
        <CurrentlyPlayingSideBar class="sideBar currentlyPlayingSideBar" />
      </div>
      <BottomPlayer />
    </template>
  </div>
</template>

<script setup>
import { ref, watch, onMounted } from "vue";
import TopBar from "@/components/TopBar.vue";
import MainContent from "@/components/content/MainContent.vue";
import BottomPlayer from "@/components/BottomPlayer.vue";
import { useRoute } from "vue-router";
import UserContentSideBar from "@/components/UserContentSideBar.vue";
import CurrentlyPlayingSideBar from "@/components/CurrentlyPlayingSideBar.vue";
import { useUserStore } from "@/store/user";

// Access the user store
const userStore = useUserStore();
const isLoading = ref(true);

// Initialize the store when the component is mounted
onMounted(async () => {
  try {
    await userStore.initialize();
  } catch (error) {
    console.error("Failed to initialize user data:", error);
  } finally {
    isLoading.value = false;
  }
});

const route = useRoute();
const searchQuery = ref(decodeURIComponent(route.params.query || ""));

// Watch for changes in the route's query parameter
watch(
  () => route.params.query,
  (newQuery) => {
    searchQuery.value = decodeURIComponent(newQuery || "");
  },
  { immediate: true },
);

function handleSearch(query) {
  searchQuery.value = query;
}

function handleSelect(item) {
  console.log("Selected:", item);
}
</script>

<style scoped>
.mainContainer {
  width: 100%;
  height: 100%;
  display: grid;
  grid-template-rows: var(--topbar-height) 1fr auto;
  grid-template-columns: 1fr;
  overflow: hidden;
  background: linear-gradient(
      180deg,
      rgba(29, 185, 84, 0.035),
      transparent 28%
    ),
    var(--bg-base);
}

.centralPanel {
  display: grid;
  grid-template-columns: 1fr;
  height: 100%;
  overflow: hidden;
  gap: 10px;
  padding: 10px;
}

/* Mobile: Hide sidebars, full-width content */
.sideBar {
  display: none;
}

.userContentSideBar {
  display: none;
  flex-direction: column;
  overflow-y: auto;
}

.currentlyPlayingSideBar {
  display: none;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
  box-sizing: border-box;
}

/* Tablet (768px+): Show left sidebar only */
@media (min-width: 768px) {
  .centralPanel {
    grid-template-columns: var(--sidebar-width-tablet) 1fr;
    gap: 12px;
    padding: 12px;
  }

  .userContentSideBar {
    display: flex;
  }

  .currentlyPlayingSideBar {
    display: none;
  }
}

/* Desktop (1024px+): Show both sidebars */
@media (min-width: 1024px) {
  .centralPanel {
    grid-template-columns: var(--sidebar-width-desktop) 1fr var(
        --sidebar-width-desktop
      );
    gap: 12px;
    padding: 12px;
  }

  .userContentSideBar {
    display: flex;
  }

  .currentlyPlayingSideBar {
    display: flex;
  }
}

/* Large Desktop (1280px+): Wider sidebars */
@media (min-width: 1280px) {
  .centralPanel {
    grid-template-columns: var(--sidebar-width-large) 1fr var(
        --sidebar-width-large
      );
  }
}

/* Loading State */
.loading-container {
  display: flex;
  flex-direction: column;
  justify-content: center;
  align-items: center;
  height: 100vh;
  width: 100%;
  gap: var(--spacing-6);
}

.loader {
  border: 5px solid rgba(255, 255, 255, 0.2);
  border-radius: var(--radius-full);
  border-top-color: var(--spotify-green);
  width: 64px;
  height: 64px;
  animation: spin 0.8s linear infinite;
}

.loading-container p {
  font-size: var(--text-lg);
  font-weight: var(--font-medium);
  color: var(--text-subdued);
}

@keyframes spin {
  0% {
    transform: rotate(0deg);
  }
  100% {
    transform: rotate(360deg);
  }
}

/* Mobile Player Height - auto handles collapse when player hidden */
@media (max-width: 767px) {
  .mainContainer {
    grid-template-rows: var(--topbar-height) 1fr auto;
  }
}
</style>
