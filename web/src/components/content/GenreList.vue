<template>
  <div class="genreListPage">
    <h1 class="pageTitle">Browse genres</h1>

    <!-- Loading State -->
    <div v-if="isLoading" class="loadingState">Loading genres...</div>

    <!-- Genre Grid -->
    <div v-else-if="genres.length > 0" class="genreGrid">
      <router-link
        v-for="genre in genres"
        :key="genre.name"
        :to="`/genre/${encodeURIComponent(genre.name)}`"
        class="genreCard"
        :style="{ background: genreBackground(genre.name) }"
      >
        <MultiSourceImage
          v-if="genre.artwork_url"
          :urls="[genre.artwork_url]"
          class="genrePoster"
          alt=""
        />
        <div class="genreIdentity">
          <span class="genreName">{{ genre.name }}</span>
          <span class="trackCount">{{
            formatTrackCount(genre.track_count)
          }}</span>
        </div>
      </router-link>
    </div>

    <!-- Empty State -->
    <div v-else class="emptyState">
      <p>No genres available</p>
    </div>
  </div>
</template>

<script setup>
import MultiSourceImage from "@/components/common/MultiSourceImage.vue";
import { genreBackground } from "@/utils/genreArtwork";
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

.genreCard {
  position: relative;
  isolation: isolate;
  overflow: hidden;
  aspect-ratio: 1;
  border-radius: 8px;
  text-decoration: none;
  background: #28282f;
}
.genrePoster {
  width: 100%;
  height: 100%;
  object-fit: cover;
  transition: transform 220ms ease;
}
.genreIdentity {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  justify-content: flex-end;
  padding: 20px;
  gap: 6px;
  background: linear-gradient(transparent 25%, #0005 55%, #000d);
}
.genreName {
  font-size: clamp(24px, 3cqw, 32px);
  font-weight: 800;
  line-height: 1.1;
  letter-spacing: -0.025em;
  color: #fff;
  text-transform: capitalize;
  overflow-wrap: anywhere;
}
.trackCount {
  font-size: var(--text-sm);
  color: #ffffffc9;
}
.genreCard:hover .genrePoster {
  transform: scale(1.045);
}
.genreCard:focus-visible {
  outline: 2px solid #fff;
  outline-offset: 4px;
}
@container (max-width: 560px) {
  .genreListPage {
    padding: 24px 16px;
  }
  .genreGrid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 12px;
  }
  .genreIdentity {
    padding: 14px;
  }
  .genreName {
    font-size: 22px;
  }
  .trackCount {
    font-size: 12px;
  }
}
@media (prefers-reduced-motion: reduce) {
  .genrePoster {
    transition: none;
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
