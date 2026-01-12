<template>
  <div class="genreListPage">
    <h1 class="pageTitle">Browse by Genre</h1>

    <!-- Loading State -->
    <div v-if="isLoading" class="loadingState">Loading genres...</div>

    <!-- Genre Grid -->
    <div v-else-if="genres.length > 0" class="genreGrid">
      <router-link
        v-for="genre in genres"
        :key="genre.name"
        :to="`/genre/${encodeURIComponent(genre.name)}`"
        class="genreCard"
      >
        <span class="genreName">{{ genre.name }}</span>
        <span class="trackCount">{{ formatTrackCount(genre.track_count) }}</span>
      </router-link>
    </div>

    <!-- Empty State -->
    <div v-else class="emptyState">
      <p>No genres available</p>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted } from "vue";
import { useRemoteStore } from "@/store/remote";

const remoteStore = useRemoteStore();
const genres = ref([]);
const isLoading = ref(true);

const formatTrackCount = (count) => {
  if (count === 1) return "1 track";
  return `${count.toLocaleString()} tracks`;
};

onMounted(async () => {
  genres.value = await remoteStore.fetchGenres();
  isLoading.value = false;
});
</script>

<style scoped>
.genreListPage {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-4);
}

.pageTitle {
  font-size: var(--text-2xl);
  font-weight: var(--font-bold);
  color: var(--text-base);
  margin: 0;
}

.genreGrid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
  gap: var(--spacing-3);
}

.genreCard {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-1);
  padding: var(--spacing-4);
  background-color: var(--bg-elevated-base);
  border-radius: var(--radius-lg);
  text-decoration: none;
  transition: background-color var(--transition-fast);
}

.genreCard:hover {
  background-color: var(--bg-elevated-highlight);
}

.genreName {
  font-size: var(--text-base);
  font-weight: var(--font-semibold);
  color: var(--text-base);
  text-transform: capitalize;
}

.trackCount {
  font-size: var(--text-sm);
  color: var(--text-subdued);
}

.loadingState,
.emptyState {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: var(--spacing-8);
  color: var(--text-subdued);
  text-align: center;
}

.emptyState p {
  margin: 0;
}
</style>
