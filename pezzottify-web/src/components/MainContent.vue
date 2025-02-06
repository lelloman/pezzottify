<template>
    <main>
        <div v-if="searchQuery" class="text-lg">
            <div v-if="loading">Loading...</div>
            <div v-else-if="results.length > 0">
                <h2 class="text-xl font-semibold mb-2">Search Results:</h2>
                <ul class="list-disc ml-5">
                    <li v-for="(result, index) in results" :key="index">{{ result }}</li>
                </ul>
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