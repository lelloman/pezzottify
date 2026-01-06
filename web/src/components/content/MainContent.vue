<template>
  <main class="mainContent">
    <div v-if="searchQuery">
      <SearchResults v-if="useOrganicSearch" :results="results" />
      <StreamingSearchResults
        v-else
        :sections="streamingSections"
        :isLoading="isStreamingLoading"
      />
    </div>
    <Track v-else-if="trackId" :trackId="trackId" />
    <Album v-else-if="albumId" :albumId="albumId" />
    <Artist v-else-if="artistId" :artistId="artistId" />
    <UserPlaylist v-else-if="playlistId" :playlistId="playlistId" />
    <UserSettings v-else-if="isSettingsRoute" />
    <UserRequests v-else-if="isRequestsRoute" />
    <HomePage v-else />
  </main>
</template>

<script setup>
import { ref, watch, computed, onUnmounted } from "vue";
import Track from "@/components/content/Track.vue";
import Album from "@/components/content/Album.vue";
import Artist from "@/components/content/Artist.vue";
import UserPlaylist from "@/components/content/UserPlaylist.vue";
import UserSettings from "@/components/content/UserSettings.vue";
import UserRequests from "@/components/content/UserRequests.vue";
import HomePage from "@/components/content/HomePage.vue";
import { useRoute } from "vue-router";
import { useDebugStore } from "@/store/debug";
import { storeToRefs } from "pinia";
import SearchResults from "./SearchResults.vue";
import StreamingSearchResults from "./StreamingSearchResults.vue";
import { streamingSearch } from "@/services/streamingSearch";

const debugStore = useDebugStore();
const { useOrganicSearch } = storeToRefs(debugStore);

const results = ref(null);
const streamingSections = ref([]);
const isStreamingLoading = ref(false);
let abortStreamingSearch = null;

const route = useRoute();
const searchQuery = ref(route.params.query || "");
const trackId = ref(route.params.trackId || "");
const artistId = ref(route.params.artistId || "");
const albumId = ref(route.params.albumId || "");
const playlistId = ref(route.params.playlistId || "");
const isSettingsRoute = computed(() => route.name === "settings");
const isRequestsRoute = computed(() => route.name === "requests");

const fetchCatalogResults = async (query, filters) => {
  const requestBody = { query, resolve: true, limit: 15 };
  if (filters) {
    requestBody.filters = filters;
  }
  try {
    const response = await fetch("/v1/content/search", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(requestBody),
    });
    return await response.json();
  } catch (error) {
    console.error("Catalog search error:", error);
    return [];
  }
};

const fetchStreamingResults = (query) => {
  // Abort any existing streaming search
  if (abortStreamingSearch) {
    abortStreamingSearch();
  }

  streamingSections.value = [];
  isStreamingLoading.value = true;

  abortStreamingSearch = streamingSearch(
    query,
    (section) => {
      streamingSections.value = [...streamingSections.value, section];
    },
    (error) => {
      console.error("Streaming search error:", error);
      isStreamingLoading.value = false;
    },
    () => {
      isStreamingLoading.value = false;
    },
  );
};

const fetchResults = async (newQuery, queryParams) => {
  if (newQuery) {
    if (useOrganicSearch.value) {
      results.value = [];
      const filters = queryParams.type ? queryParams.type.split(",") : null;
      results.value = await fetchCatalogResults(newQuery, filters);
    } else {
      fetchStreamingResults(newQuery);
    }
  } else {
    results.value = [];
    streamingSections.value = [];
  }
};

watch(
  [() => route.params.query, () => route.query],
  ([newQuery, newQueryParams]) => {
    console.log("MainContent newQueryParams:");
    console.log(newQueryParams);
    searchQuery.value = newQuery || "";
    fetchResults(newQuery, route.query);
  },
  { immediate: true },
);

// Cleanup on unmount
onUnmounted(() => {
  if (abortStreamingSearch) {
    abortStreamingSearch();
  }
});
watch(
  () => route.params.trackId,
  (newTrackId) => {
    trackId.value = newTrackId || "";
  },
  { immediate: true },
);
watch(
  () => route.params.artistId,
  (newArtistId) => {
    artistId.value = newArtistId || "";
  },
  { immediate: true },
);
watch(
  () => route.params.albumId,
  (newAlbumId) => {
    albumId.value = newAlbumId || "";
  },
  { immediate: true },
);
watch(
  () => route.params.playlistId,
  (newPlaylistId) => {
    playlistId.value = newPlaylistId || "";
  },
  { immediate: true },
);
</script>

<style>
.mainContent {
  flex: 1;
  overflow: auto;
  background-color: var(--panel-on-bg);
  border-radius: var(--panel-border-radius);
  padding: 16px;
  margin-left: 8px;
  margin-right: 16px;
  color: var(--text-base);
}
</style>
