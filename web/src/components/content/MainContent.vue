<template>
  <main class="mainContent">
    <div v-if="searchQuery">
      <SearchResults :results="results" />
    </div>
    <Track v-else-if="trackId" :trackId="trackId" />
    <Album v-else-if="albumId" :albumId="albumId" />
    <Artist v-else-if="artistId" :artistId="artistId" />
    <UserPlaylist v-else-if="playlistId" :playlistId="playlistId" />
    <UserSettings v-else-if="isSettingsRoute" />
    <div v-else>
      <h1 class="text-2xl font-bold mb-4">Welcome to Home</h1>
      <p>This is your home content.</p>
      Showing track {{ trackId }}.
    </div>
  </main>
</template>

<script setup>
import { ref, watch, computed } from 'vue';
import Track from '@/components/content/Track.vue';
import Album from '@/components/content/Album.vue';
import Artist from '@/components/content/Artist.vue';
import UserPlaylist from '@/components/content/UserPlaylist.vue';
import UserSettings from '@/components/content/UserSettings.vue';
import { useRoute } from 'vue-router';
import SearchResults from './SearchResults.vue';

const results = ref(null);

const route = useRoute();
const searchQuery = ref(route.params.query || '');
const trackId = ref(route.params.trackId || '');
const artistId = ref(route.params.artistId || '');
const albumId = ref(route.params.albumId || '');
const playlistId = ref(route.params.playlistId || '');
const isSettingsRoute = computed(() => route.name === 'settings');

const fetchResults = async (newQuery, queryParams) => {
  console.log("watch query? " + newQuery)
  if (newQuery) {
    results.value = [];
    const requestBody = { query: newQuery, resolve: true };

    const filters = queryParams.type ? queryParams.type.split(',') : null;
    if (filters) {
      requestBody.filters = filters;
    }
    try {
      const response = await fetch('/v1/content/search', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify(requestBody),
      });
      const data = await response.json();
      console.log("search response: ");
      console.log(data);
      results.value = data;
    } catch (error) {
      console.error('Search error:', error);
    }
  } else {
    results.value = [];
  }
}
watch(
  [
    () => route.params.query,
    () => route.query,
  ],
  ([newQuery, newQueryParams], [oldQuery, oldQueryParams]) => {
    console.log("MainContent newQueryParams:");
    console.log(newQueryParams);
    searchQuery.value = newQuery || '';
    fetchResults(newQuery, route.query);
  },
  { immediate: true }
);
watch(
  () => route.params.trackId,
  (newTrackId) => { trackId.value = newTrackId || ''; },
  { immediate: true }
)
watch(
  () => route.params.artistId,
  (newArtistId) => { artistId.value = newArtistId || ''; },
  { immediate: true }
)
watch(
  () => route.params.albumId,
  (newAlbumId) => { albumId.value = newAlbumId || ''; },
  { immediate: true }
)
watch(
  () => route.params.playlistId,
  (newPlaylistId) => { playlistId.value = newPlaylistId || ''; },
  { immediate: true }
)
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
