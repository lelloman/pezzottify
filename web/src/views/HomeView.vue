<template>
  <div class="mainContainer">
    <div v-if="isLoading" class="loading-container">
      <div class="loader"></div>
      <p>Loading your content...</p>
    </div>
    <template v-else>
      <TopBar @search="handleSearch" :initialQuery="searchQuery" />
      <div
        class="centralPanel"
        :class="{
          libraryExpanded: libraryLayout === 'wide',
          mobileLibrary: mobileSurface === 'library',
          mobileQueue: mobileSurface === 'queue',
        }"
        :style="libraryStyle"
      >
        <nav class="mobileNavigation" aria-label="Mobile navigation">
          <button
            v-for="surface in mobileSurfaces"
            :key="surface.id"
            :aria-pressed="mobileSurface === surface.id"
            @click="mobileSurface = surface.id"
          >
            {{ surface.label }}
          </button>
        </nav>
        <UserContentSideBar
          @layout-change="libraryLayout = $event"
          :requested-layout="libraryLayout"
          class="sideBar userContentSideBar"
        />
        <MainContent class="mainContentPanel" :search-query="searchQuery" />
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
const mobileSurface = ref("browse");
const mobileSurfaces = [
  { id: "browse", label: "Browse" },
  { id: "library", label: "Your library" },
  { id: "queue", label: "Queue" },
];
const libraryStyle = computed(() =>
  libraryLayout.value === "collapsed" ? { "--library-width": "72px" } : {},
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
watch(
  () => route.fullPath,
  () => {
    mobileSurface.value = "browse";
    if (libraryLayout.value === "wide") libraryLayout.value = "normal";
  },
);
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

.mobileNavigation {
  display: none;
}

/* Mobile: one selected surface fills the content area. */
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
    grid-template-columns:
      var(--library-width, var(--sidebar-width-tablet))
      minmax(0, 1fr);
  }

  .userContentSideBar {
    display: flex;
  }

  .currentlyPlayingSideBar {
    display: none;
  }
}

@media (min-width: 768px) {
  .libraryExpanded .userContentSideBar {
    grid-column: 1 / span 2;
  }
  .libraryExpanded .mainContentPanel {
    display: none;
  }
}

/* Keep the content spacious before adding the queue column. */
@media (min-width: 1024px) {
  .centralPanel {
    grid-template-columns:
      var(--library-width, var(--sidebar-width-desktop))
      minmax(0, 1fr);
  }
}

@media (min-width: 1280px) {
  .centralPanel {
    grid-template-columns:
      var(--library-width, var(--sidebar-width-desktop))
      minmax(0, 1fr) 280px;
  }
  .currentlyPlayingSideBar {
    display: flex;
  }
}

@media (min-width: 1600px) {
  .centralPanel {
    grid-template-columns:
      var(--library-width, var(--sidebar-width-large))
      minmax(0, 1fr) 320px;
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
  .centralPanel {
    grid-template-rows: auto minmax(0, 1fr);
  }
  .mobileNavigation {
    display: flex;
    gap: 8px;
    padding: 0 8px;
  }
  .mobileNavigation button {
    border: 0;
    border-radius: 999px;
    background: #232323;
    color: var(--text-base);
    padding: 8px 16px;
    min-height: 40px;
    font: inherit;
    font-size: 14px;
    cursor: pointer;
  }
  .mobileNavigation button[aria-pressed="true"] {
    background: #fff;
    color: #000;
  }
  .mobileNavigation button:focus-visible {
    outline: 2px solid #fff;
    outline-offset: 2px;
  }
  .mobileLibrary .mainContentPanel,
  .mobileQueue .mainContentPanel {
    display: none;
  }
  .mobileLibrary .userContentSideBar,
  .mobileQueue .currentlyPlayingSideBar {
    display: flex;
    grid-row: 2;
    min-height: 0;
  }

  .mainContainer {
    grid-template-rows: var(--topbar-height) minmax(0, 1fr) auto;
  }
}
</style>
