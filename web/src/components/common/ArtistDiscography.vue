<template>
  <div class="discographyContainer">
    <div class="header">
      <h1>Discography</h1>
      <div class="sortSelector">
        <label>Sort by:</label>
        <select v-model="sortOrder" @change="resetAndLoad">
          <option value="popularity">Popularity</option>
          <option value="release_date">Release Date</option>
        </select>
      </div>
    </div>

    <div v-if="albums.length > 0" class="albumsContainer" ref="albumsContainerRef">
      <AlbumCard v-for="album in albums" :key="album.id" :album="album" />
    </div>

    <div v-if="isLoading" class="loadingIndicator">Loading...</div>

    <div v-if="error" class="error">{{ error }}</div>

    <div v-if="!isLoading && !hasMore && albums.length > 0" class="endMessage">
      End of discography ({{ total }} albums)
    </div>

    <div ref="sentinelRef" class="sentinel"></div>
  </div>
</template>

<script setup>
import { onMounted, onUnmounted, watch, ref } from "vue";
import AlbumCard from "@/components/common/AlbumCard.vue";
import { useRemoteStore } from "@/store/remote";

const props = defineProps({
  artistId: {
    type: String,
    required: true,
  },
});

const PAGE_SIZE = 50;

const remoteStore = useRemoteStore();
const albums = ref([]);
const total = ref(0);
const hasMore = ref(true);
const error = ref(null);
const isLoading = ref(false);
const sortOrder = ref("popularity");
const offset = ref(0);

const sentinelRef = ref(null);
let observer = null;

const loadMore = async () => {
  if (isLoading.value || !hasMore.value) return;

  isLoading.value = true;
  error.value = null;

  try {
    const response = await remoteStore.fetchArtistDiscography(props.artistId, {
      limit: PAGE_SIZE,
      offset: offset.value,
      sort: sortOrder.value,
    });

    if (response) {
      albums.value = [...albums.value, ...response.albums];
      total.value = response.total;
      hasMore.value = response.has_more;
      offset.value += response.albums.length;
    } else {
      error.value = "Error fetching artist albums";
    }
  } catch {
    error.value = "Error fetching artist albums";
  }

  isLoading.value = false;
};

const resetAndLoad = () => {
  albums.value = [];
  offset.value = 0;
  hasMore.value = true;
  error.value = null;
  loadMore();
};

const setupIntersectionObserver = () => {
  if (observer) {
    observer.disconnect();
  }

  observer = new IntersectionObserver(
    (entries) => {
      if (entries[0].isIntersecting && !isLoading.value && hasMore.value) {
        loadMore();
      }
    },
    {
      rootMargin: "200px",
    },
  );

  if (sentinelRef.value) {
    observer.observe(sentinelRef.value);
  }
};

watch(
  () => props.artistId,
  () => {
    resetAndLoad();
  },
);

onMounted(() => {
  loadMore();
  setupIntersectionObserver();
});

onUnmounted(() => {
  if (observer) {
    observer.disconnect();
  }
});
</script>

<style scoped>
.discographyContainer {
  display: flex;
  flex-direction: column;
}

.header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;
}

.header h1 {
  margin: 0;
}

.sortSelector {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--text-subtle, #b3b3b3);
  font-size: var(--text-sm, 14px);
}

.sortSelector select {
  padding: 6px 12px;
  border-radius: var(--radius-md, 4px);
  border: 1px solid var(--border-default, #333);
  background: var(--bg-base, #121212);
  color: var(--text-base, #fff);
  font-size: var(--text-sm, 14px);
  cursor: pointer;
}

.sortSelector select:focus {
  outline: none;
  border-color: var(--spotify-green, #1db954);
}

.sortSelector select option {
  background: var(--bg-base, #121212);
  color: var(--text-base, #fff);
}

.albumsContainer {
  display: grid;
  gap: 16px;
  grid-template-columns: repeat(1, 1fr);
  justify-items: start;
}

@media (min-width: 1000px) {
  .albumsContainer {
    grid-template-columns: repeat(2, 1fr);
  }
}

@media (min-width: 1500px) {
  .albumsContainer {
    grid-template-columns: repeat(3, 1fr);
  }
}

.loadingIndicator {
  text-align: center;
  padding: 16px;
  color: var(--text-subtle, #b3b3b3);
}

.error {
  text-align: center;
  padding: 16px;
  color: var(--error, #e91429);
}

.endMessage {
  text-align: center;
  padding: 16px;
  color: var(--text-subdued, #6a6a6a);
  font-size: var(--text-sm, 14px);
}

.sentinel {
  height: 1px;
}
</style>
