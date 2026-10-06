<template>
  <div class="genreListPage">
    <h1 class="pageTitle">Browse genres</h1>

    <!-- Loading State -->
    <div v-if="isLoading" class="loadingState">Loading genres...</div>

    <!-- Genre Grid -->
    <div v-else-if="genres.length > 0" class="genreGrid">
      <GenreCard v-for="genre in genres" :key="genre.name" :genre="genre" />
    </div>

    <!-- Empty State -->
    <div v-else class="emptyState">
      <p>No genres available</p>
    </div>
  </div>
</template>

<script setup>
import GenreCard from "@/components/common/GenreCard.vue";
import { ref, onMounted } from "vue";
import { useRemoteStore } from "@/store/remote";

const remoteStore = useRemoteStore();
const genres = ref([]);
const isLoading = ref(true);

onMounted(async () => {
  genres.value = await remoteStore.fetchGenres();
  isLoading.value = false;
});
</script>

<style scoped>
.genreListPage {
  display: flex;
  flex-direction: column;
  gap: 24px;
  padding: 24px;
}

.pageTitle {
  font-size: var(--text-2xl);
  font-weight: var(--font-bold);
  color: var(--text-base);
  margin: 0;
}

.genreGrid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(min(100%, 210px), 1fr));
  gap: 20px;
}

@container (max-width: 560px) {
  .genreListPage {
    padding: 24px 16px;
  }
  .genreGrid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 12px;
  }
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
