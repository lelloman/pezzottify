<template>
  <main ref="mainEl" class="mainContent">
    <keep-alive :max="5">
      <SearchWrapper
        v-if="searchQuery"
        :key="'search-' + searchQuery"
        :query="searchQuery"
        :useOrganicSearch="useOrganicSearch"
        :results="results"
        :streamingSections="streamingSections"
        :isStreamingLoading="isStreamingLoading"
        :searchError="searchError"
      />
      <Track v-else-if="trackId" :key="'track-' + trackId" :trackId="trackId" />
      <Work v-else-if="workId" :key="'work-' + workId" :workId="workId" />
      <Album v-else-if="albumId" :key="'album-' + albumId" :albumId="albumId" />
      <Artist
        v-else-if="artistId"
        :key="'artist-' + artistId"
        :artistId="artistId"
      />
      <UserPlaylist
        v-else-if="playlistId"
        :key="'playlist-' + playlistId"
        :playlistId="playlistId"
      />
      <UserSettings v-else-if="isSettingsRoute" />
      <UserRequests v-else-if="isRequestsRoute" />
      <GenreList v-else-if="isGenresRoute" />
      <GenreDetail
        v-else-if="genreName"
        :key="'genre-' + genreName"
        :genreName="genreName"
      />
      <DevicesView v-else-if="isDevicesRoute" />
      <Steering v-else-if="isSteeringRoute" />
      <NowPlaying v-else-if="isNowPlayingRoute" />
      <HomePage v-else />
    </keep-alive>
  </main>
</template>

<script setup>
import { ref, watch, computed, onUnmounted, nextTick } from "vue";
import Track from "@/components/content/Track.vue";
import Work from "@/components/content/Work.vue";
import Album from "@/components/content/Album.vue";
import Artist from "@/components/content/Artist.vue";
import UserPlaylist from "@/components/content/UserPlaylist.vue";
import UserSettings from "@/components/content/UserSettings.vue";
import UserRequests from "@/components/content/UserRequests.vue";
import HomePage from "@/components/content/HomePage.vue";
import GenreList from "@/components/content/GenreList.vue";
import GenreDetail from "@/components/content/GenreDetail.vue";
import DevicesView from "@/components/content/DevicesView.vue";
import NowPlaying from "@/components/NowPlaying.vue";
import Steering from "@/components/content/Steering.vue";
import { useRoute } from "vue-router";
import { useDebugStore } from "@/store/debug";
import { storeToRefs } from "pinia";
import SearchWrapper from "./SearchWrapper.vue";
import { streamingSearch } from "@/services/streamingSearch";
import { authenticatedFetch } from "@/services/authenticatedFetch.js";

const debugStore = useDebugStore();
const { useOrganicSearch, excludeUnavailable } = storeToRefs(debugStore);

const results = ref([]);
const streamingSections = ref([]);
const isStreamingLoading = ref(false);
let abortStreamingSearch = null;

const route = useRoute();
const searchQuery = ref(route.params.query || "");
const trackId = ref(route.params.trackId || "");
const workId = computed(() => route.params.workId || "");
const artistId = ref(route.params.artistId || "");
const albumId = ref(route.params.albumId || "");
const playlistId = ref(route.params.playlistId || "");
const isSettingsRoute = computed(() => route.name === "settings");
const isRequestsRoute = computed(() => route.name === "requests");
const isGenresRoute = computed(() => route.name === "genres");
const isDevicesRoute = computed(() => route.name === "devices");
const isNowPlayingRoute = computed(() => route.name === "now-playing");
const isSteeringRoute = computed(() => route.name === "steering");
const genreName = ref(route.params.genreName || "");

// Scroll position persistence across navigations
const mainEl = ref(null);
const scrollPositions = new Map();

function currentRouteKey() {
  if (searchQuery.value) return "search-" + searchQuery.value;
  if (trackId.value) return "track-" + trackId.value;
  if (workId.value) return "work-" + workId.value;
  if (albumId.value) return "album-" + albumId.value;
  if (artistId.value) return "artist-" + artistId.value;
  if (playlistId.value) return "playlist-" + playlistId.value;
  if (genreName.value) return "genre-" + genreName.value;
  if (isSettingsRoute.value) return "settings";
  if (isRequestsRoute.value) return "requests";
  if (isGenresRoute.value) return "genres";
  if (isDevicesRoute.value) return "devices";
  if (isSteeringRoute.value) return "steering";
  if (isNowPlayingRoute.value) return "now-playing";
  return "home";
}

let previousRouteKey = currentRouteKey();

watch(
  () => route.fullPath,
  () => {
    // Save scroll position for the page we're leaving
    if (mainEl.value) {
      scrollPositions.set(previousRouteKey, mainEl.value.scrollTop);
    }

    nextTick(() => {
      const newKey = currentRouteKey();
      previousRouteKey = newKey;

      // Restore scroll position for the page we're navigating to
      if (mainEl.value) {
        const saved = scrollPositions.get(newKey);
        mainEl.value.scrollTop = saved != null ? saved : 0;
      }
    });
  },
);

const fetchCatalogResults = async (query, filters) => {
  const requestBody = {
    query,
    resolve: true,
    limit: 15,
    exclude_unavailable: excludeUnavailable.value,
    search_mode: "expanded",
  };
  if (filters) {
    requestBody.filters = filters;
  }
  const response = await authenticatedFetch("/v1/content/search", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(requestBody),
  });
  if (!response.ok) throw new Error(`Search failed: ${response.status}`);
  return await response.json();
};

const fetchStreamingResults = (query, version) => {
  // Abort any existing streaming search
  if (abortStreamingSearch) {
    abortStreamingSearch();
  }

  streamingSections.value = [];
  isStreamingLoading.value = true;

  abortStreamingSearch = streamingSearch(
    query,
    (section) => {
      if (version !== searchVersion) return;
      streamingSections.value = [...streamingSections.value, section];
    },
    (error) => {
      if (version !== searchVersion) return;
      console.error("Streaming search error:", error);
      searchError.value = true;
      isStreamingLoading.value = false;
    },
    () => {
      if (version !== searchVersion) return;
      isStreamingLoading.value = false;
    },
    { excludeUnavailable: excludeUnavailable.value },
  );
};

let searchVersion = 0;
const searchError = ref(false);
const fetchResults = async (newQuery, queryParams) => {
  const version = ++searchVersion;
  abortStreamingSearch?.();
  searchError.value = false;
  if (newQuery) {
    if (useOrganicSearch.value) {
      results.value = [];
      isStreamingLoading.value = true;
      const filters = queryParams.type ? queryParams.type.split(",") : null;
      try {
        const fetched = await fetchCatalogResults(newQuery, filters);
        if (version === searchVersion)
          results.value = Array.isArray(fetched) ? fetched : [];
      } catch (error) {
        if (version === searchVersion) searchError.value = true;
        console.error("Catalog search error:", error);
      } finally {
        if (version === searchVersion) isStreamingLoading.value = false;
      }
    } else {
      fetchStreamingResults(newQuery, version);
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
watch(
  () => route.params.genreName,
  (newGenreName) => {
    genreName.value = newGenreName || "";
  },
  { immediate: true },
);
</script>

<style>
.mainContent {
  flex: 1;
  min-width: 0;
  container-type: inline-size;
  overflow: auto;
  background: var(--surface-panel);
  border: 0;
  border-radius: 8px;
  padding: 0;
  margin: 0;
  color: var(--text-base);
  min-height: 0;
}

.mainContent
  > :not(.homePage):not(.nowPlaying):not(.detailPage):not(.detailPageHost) {
  padding: clamp(18px, 2vw, 30px);
}
</style>
