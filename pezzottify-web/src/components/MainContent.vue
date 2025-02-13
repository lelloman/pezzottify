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
            <div v-else class="p-4 border rounded-lg shadow-sm bg-gray-50">
              <p>Unknown result type</p>
            </div>
          </div>
        </div>
      </div>
      <div v-else>No results found for "{{ searchQuery }}"</div>
    </div>
    <div v-else-if="trackId">
      Showing track {{ trackId }}.
    </div>
    <div v-else>
      <h1 class="text-2xl font-bold mb-4">Welcome to Home</h1>
      <p>This is your home content.</p>
      Showing track {{ trackId }}.
    </div>
  </main>
</template>

<script setup>
import { ref, watch } from 'vue';
import AlbumResult from './search/AlbumResult.vue';
import ArtistResult from './search/ArtistResult.vue';
import TrackResult from './search/TrackResult.vue';
import { useRoute } from 'vue-router';

const results = ref([]);
const loading = ref(false);

const route = useRoute();
const searchQuery = ref(route.params.query || '');
const trackId = ref(route.params.trackId || '');

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
</script>


<style>
.mainContent {
  flex: 1;
  overflow: auto;
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
