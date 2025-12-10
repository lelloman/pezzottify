<template>
  <main class="mainContent">
    <div v-if="searchQuery">
      <SearchResults
        :results="results"
        :externalResults="externalResults"
        :externalLimits="externalLimits"
        :showExternalSearch="showExternalSearch"
        @request-album="handleRequestAlbum"
      />
    </div>
    <Track v-else-if="trackId" :trackId="trackId" />
    <Album v-else-if="albumId" :albumId="albumId" />
    <Artist v-else-if="artistId" :artistId="artistId" />
    <UserPlaylist v-else-if="playlistId" :playlistId="playlistId" />
    <UserSettings v-else-if="isSettingsRoute" />
    <UserRequests v-else-if="isRequestsRoute" />
    <div v-else>
      <h1 class="text-2xl font-bold mb-4">Welcome to Home</h1>
      <p>This is your home content.</p>
      Showing track {{ trackId }}.
    </div>
  </main>
</template>

<script setup>
import { ref, watch, computed } from "vue";
import Track from "@/components/content/Track.vue";
import Album from "@/components/content/Album.vue";
import Artist from "@/components/content/Artist.vue";
import UserPlaylist from "@/components/content/UserPlaylist.vue";
import UserSettings from "@/components/content/UserSettings.vue";
import UserRequests from "@/components/content/UserRequests.vue";
import { useRoute } from "vue-router";
import SearchResults from "./SearchResults.vue";
import { useUserStore } from "@/store/user";

const userStore = useUserStore();

const results = ref(null);
const externalResults = ref(null);
const externalLimits = ref(null);

const route = useRoute();
const searchQuery = ref(route.params.query || "");
const trackId = ref(route.params.trackId || "");
const artistId = ref(route.params.artistId || "");
const albumId = ref(route.params.albumId || "");
const playlistId = ref(route.params.playlistId || "");
const isSettingsRoute = computed(() => route.name === "settings");
const isRequestsRoute = computed(() => route.name === "requests");

// Check if external search should be shown
const showExternalSearch = computed(() => {
  return userStore.canRequestContent && userStore.isExternalSearchEnabled;
});

// Determine external search type based on selected filters
const getExternalSearchType = (filters) => {
  if (!filters || filters.length === 0) {
    return "album"; // Default to album
  }
  // If only artist is selected, search artists
  if (filters.length === 1 && filters[0] === "artist") {
    return "artist";
  }
  // If album is selected (alone or with others), search albums
  if (filters.includes("album")) {
    return "album";
  }
  // If only track is selected, default to album (tracks not supported in external)
  return "album";
};

const fetchCatalogResults = async (query, filters) => {
  const requestBody = { query, resolve: true };
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

const fetchExternalResults = async (query, type) => {
  try {
    const response = await fetch(
      `/v1/download/search?q=${encodeURIComponent(query)}&type=${type}`,
    );
    if (!response.ok) {
      console.error("External search error:", response.status);
      return null;
    }
    return await response.json();
  } catch (error) {
    console.error("External search error:", error);
    return null;
  }
};

const fetchExternalLimits = async () => {
  try {
    const response = await fetch("/v1/download/limits");
    if (!response.ok) {
      return null;
    }
    return await response.json();
  } catch (error) {
    console.error("Error fetching limits:", error);
    return null;
  }
};

const fetchResults = async (newQuery, queryParams) => {
  if (newQuery) {
    results.value = [];
    externalResults.value = null;
    externalLimits.value = null;

    const filters = queryParams.type ? queryParams.type.split(",") : null;

    // Fetch both searches in parallel
    const catalogPromise = fetchCatalogResults(newQuery, filters);

    if (showExternalSearch.value) {
      const externalType = getExternalSearchType(filters);
      const externalPromise = fetchExternalResults(newQuery, externalType);
      const limitsPromise = fetchExternalLimits();

      // Run all in parallel
      const [catalogData, extResults, limits] = await Promise.all([
        catalogPromise,
        externalPromise,
        limitsPromise,
      ]);

      results.value = catalogData;
      externalResults.value = extResults || { results: [] };
      externalLimits.value = limits;
    } else {
      results.value = await catalogPromise;
    }
  } else {
    results.value = [];
    externalResults.value = null;
    externalLimits.value = null;
  }
};

const handleRequestAlbum = async (album) => {
  try {
    const response = await fetch("/v1/download/request/album", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        album_id: album.id,
        album_name: album.name,
        artist_name: album.artist_name,
      }),
    });
    if (response.ok) {
      // Refresh limits and mark item as in_queue
      externalLimits.value = await fetchExternalLimits();
      // Update the result to show it's now in queue
      if (externalResults.value && externalResults.value.results) {
        externalResults.value = {
          ...externalResults.value,
          results: externalResults.value.results.map((r) =>
            r.id === album.id ? { ...r, in_queue: true } : r,
          ),
        };
      }
    } else {
      console.error("Failed to request album:", response.status);
    }
  } catch (error) {
    console.error("Error requesting album:", error);
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
