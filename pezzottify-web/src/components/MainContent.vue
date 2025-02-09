<template>
  <main class="mainContent">
    <div v-if="searchQuery">
      <div v-if="loading">Loading...</div>
      <div v-else-if="results.length > 0">
        <h2 class="text-xl font-semibold mb-2">Search Results:</h2>
        <div class="list-none ml-5">
          <div v-for="(result, index) in results" :key="index">

            <AlbumResult v-if="result.type === 'Album'" :result="result" />

            <div v-else-if="result.type === 'Track'" class="p-4 border rounded-lg shadow-sm bg-blue-50">
              <h3 class="font-bold">{{ result.name }}</h3>
              <p> Track {{ result.name }}</p>
              <button @click="handleAction(result)"
                class="mt-2 px-4 py-2 bg-blue-500 text-white rounded hover:bg-blue-600">Action</button>
            </div>
            <div v-else-if="result.type === 'Artist'" class="p-4 border rounded-lg shadow-sm bg-green-50">
              <p> Artist {{ result.name }}</p>
            </div>
            <div v-else class="p-4 border rounded-lg shadow-sm bg-gray-50">
              <p>Unknown result type</p>
            </div>
          </div>
        </div>
      </div>
      <div v-else>No results found for "{{ searchQuery }}"</div>
    </div>
    <div v-else>
      <h1 class="text-2xl font-bold mb-4">Welcome to Home</h1>
      <p>This is your home content.</p>
    </div>
  </main>
</template>

<script setup>
import { ref, watch } from 'vue';
import AlbumResult from './search/AlbumResult.vue';

const props = defineProps({ searchQuery: String });
const results = ref([]);
const loading = ref(false);

watch(() => props.searchQuery, async (newQuery) => {
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
      console.log("search response: " + JSON.stringify(data));
      results.value = data;
    } catch (error) {
      console.error('Search error:', error);
    } finally {
      loading.value = false;
    }
  } else {
    results.value = [];
  }
});
</script>


<style>
.mainContent {
  flex: 1;
  overflow: auto;
}
</style>
