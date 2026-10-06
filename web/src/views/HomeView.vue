<template>
  <div class="mainContainer">
    <div v-if="isLoading" class="loading-container">
      <div class="loader"></div>
      <p>Loading your content...</p>
    </div>
    <template v-else>
      <TopBar @search="handleSearch" :initialQuery="searchQuery" />
      <div class="centralPanel" :style="libraryStyle">
        <UserContentSideBar
          @layout-change="libraryLayout = $event"
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
import { ref, computed, watch, onMounted } from "vue";
import TopBar from "@/components/TopBar.vue";
import MainContent from "@/components/content/MainContent.vue";
import BottomPlayer from "@/components/BottomPlayer.vue";
import { useRoute } from "vue-router";
import UserContentSideBar from "@/components/UserContentSideBar.vue";
import CurrentlyPlayingSideBar from "@/components/CurrentlyPlayingSideBar.vue";
import { useUserStore } from "@/store/user";

// Access the user store
const userStore = useUserStore();
const libraryLayout = ref("normal");
const libraryStyle = computed(() =>
  libraryLayout.value === "normal"
    ? {}
    : {
        "--library-width":
          libraryLayout.value === "collapsed" ? "72px" : "min(400px, 40vw)",
      },
);
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
</script>

<style scoped>
.mainContainer {
  width: 100%;
  height: 100%;
  display: grid;
  grid-template-rows: var(--topbar-height) minmax(0, 1fr) auto;
  grid-template-columns: minmax(0, 1fr);
  overflow: hidden;
  background: var(--bg-base);
}

.centralPanel {
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  height: 100%;
  overflow: hidden;
  min-width: 0;
  min-height: 0;
  gap: 8px;
  padding: 0 8px;
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
    grid-template-columns: var(
        --library-width,
        var(--sidebar-width-tablet)
      ) minmax(0, 1fr);
  }

  .userContentSideBar {
    display: flex;
  }

  .currentlyPlayingSideBar {
    display: none;
  }
}

/* Keep the content spacious before adding the queue column. */
@media (min-width: 1024px) {
  .centralPanel {
    grid-template-columns: var(
        --library-width,
        var(--sidebar-width-desktop)
      ) minmax(0, 1fr);
  }
}

@media (min-width: 1280px) {
  .centralPanel {
    grid-template-columns: var(
        --library-width,
        var(--sidebar-width-desktop)
      ) minmax(0, 1fr) 280px;
  }
  .currentlyPlayingSideBar {
    display: flex;
  }
}

@media (min-width: 1600px) {
  .centralPanel {
    grid-template-columns: var(
        --library-width,
        var(--sidebar-width-large)
      ) minmax(0, 1fr) 320px;
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
    grid-template-rows: var(--topbar-height) minmax(0, 1fr) auto;
  }
}
</style>
