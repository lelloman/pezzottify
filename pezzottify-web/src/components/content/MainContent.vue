<template>
  <main class="mainContent">
    <div v-if="searchQuery">
      <div v-if="loading">Loading...</div>
      <div v-else-if="results.length > 0">
        <div class="searchResultsContainer">
          <div v-for="(result, index) in results" :key="index">
            <AlbumResult v-if="result.type === 'Album'" :result="result" />
            <ArtistResult v-else-if="result.type === 'Artist'" :result="result" />
            <TrackResult v-else-if="result.type === 'Track'" :result="result" />
            <div v-else class="">
              <p>Unknown result type</p>
            </div>
          </div>
        </div>
      </div>
      <div v-else>No results found for "{{ searchQuery }}"</div>
    </div>
    <Track v-else-if="trackId" :trackId="trackId" />
    <Album v-else-if="albumId" :albumId="albumId" />
    <Artist v-else-if="artistId" :artistId="artistId" />
    <div v-else>
      <h1 class="text-2xl font-bold mb-4">Welcome to Home</h1>
      <p>This is your home content.</p>
      Showing track {{ trackId }}.
    </div>
  </main>
</template>

<script setup>
import { ref, watch } from 'vue';
import AlbumResult from '@/components/search/AlbumResult.vue';
import ArtistResult from '@/components/search/ArtistResult.vue';
import TrackResult from '@/components/search/TrackResult.vue';
import Track from '@/components/content/Track.vue';
import Album from '@/components/content/Album.vue';
import Artist from '@/components/content/Artist.vue';
import { useRoute } from 'vue-router';

const results = ref([]);
const loading = ref(false);

const route = useRoute();
const searchQuery = ref(route.params.query || '');
const trackId = ref(route.params.trackId || '');
const artistId = ref(route.params.artistId || '');
const albumId = ref(route.params.albumId || '');

const fetchResults = async (newQuery) => {
  console.log("watch query? " + newQuery)
  if (newQuery) {
    loading.value = true;
    results.value = [];
    try {
      const response = await fetch('/v1/content/search', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({ query: newQuery, resolve: true }),
      });
      const data = await response.json();
      //console.log("search response: " + JSON.stringify(data));
      results.value = data;
    } catch (error) {
      console.error('Search error:', error);
    } finally {
      loading.value = false;
    }
  } else {
    results.value = [];
  }
}
watch(
  () => route.params.query,
  (newQuery) => {
    searchQuery.value = newQuery || '';
    fetchResults(newQuery);
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
  margin-bottom: 16px;
}

.searchResultsContainer {
  display: grid;
  gap: 16px;
  grid-template-columns: repeat(1, 1fr);
}

@media (min-width: 1000px) {
  .searchResultsContainer {
    grid-template-columns: repeat(2, 1fr);
  }
}

@media (min-width: 1500px) {
  .searchResultsContainer {
    grid-template-columns: repeat(3, 1fr);
  }
}
</style>
