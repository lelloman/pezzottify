<template>
  <div class="wrapper">
    <div class="filtersSection">
      <span class="filtersLabel">Search for:</span>
      <div class="filtersContainer">
        <div
          :class="{
            filter: true,
            selectedFilter: isAllSelected,
            scaleClickFeedback: true,
          }"
          @click.stop="selectAll"
        >
          All
        </div>
        <div
          :class="{
            filter: true,
            selectedFilter: selectedFilters.indexOf('album') > -1 && !isAllSelected,
            scaleClickFeedback: true,
          }"
          @click.stop="toggleFilter('album')"
        >
          Albums
        </div>
        <div
          :class="{
            filter: true,
            selectedFilter: selectedFilters.indexOf('artist') > -1 && !isAllSelected,
            scaleClickFeedback: true,
          }"
          @click.stop="toggleFilter('artist')"
        >
          Artists
        </div>
        <div
          :class="{
            filter: true,
            selectedFilter: selectedFilters.indexOf('track') > -1 && !isAllSelected,
            scaleClickFeedback: true,
          }"
          @click.stop="toggleFilter('track')"
        >
          Tracks
        </div>
      </div>
    </div>

    <!-- Results Section -->
    <div class="resultsSection">
      <h2 class="sectionTitle">Results</h2>
      <div v-if="results && results.length > 0" class="searchResultsContainer">
        <div v-for="(result, index) in results" :key="index" class="searchResult">
          <AlbumResult v-if="result.type === 'Album'" :result="result" />
          <ArtistResult v-else-if="result.type === 'Artist'" :result="result" />
          <TrackResult v-else-if="result.type === 'Track'" :result="result" />
        </div>
      </div>
      <p v-else class="noResults">No results found</p>
    </div>
  </div>
</template>

<script setup>
import { ref, watch, computed, defineProps } from "vue";
import AlbumResult from "@/components/search/AlbumResult.vue";
import ArtistResult from "@/components/search/ArtistResult.vue";
import TrackResult from "@/components/search/TrackResult.vue";
import { useRoute, useRouter } from "vue-router";

const ALL_FILTERS = ["album", "artist", "track"];

const props = defineProps({
  results: {
    type: Array,
    required: true,
  },
});

const selectedFilters = ref([...ALL_FILTERS]);
const isLoading = ref(true);

const router = useRouter();
const route = useRoute();

// Computed to check if all filters are selected
const isAllSelected = computed(() => {
  return ALL_FILTERS.every((f) => selectedFilters.value.includes(f));
});

const selectAll = () => {
  selectedFilters.value = [...ALL_FILTERS];
};

const toggleFilter = (filter) => {
  if (selectedFilters.value.indexOf(filter) > -1) {
    if (selectedFilters.value.length > 1) {
      selectedFilters.value = selectedFilters.value.filter((f) => f !== filter);
    }
  } else {
    selectedFilters.value = [...selectedFilters.value, filter];
  }
};

watch(props.results, (newResults) => {
  if (newResults) {
    isLoading.value = false;
  }
});
watch(selectedFilters, (newFilters) => {
  if (newFilters) {
    if (newFilters.length === ALL_FILTERS.length) {
      // remove query parameters when all selected
      router.push({ query: {} });
    } else {
      const args = newFilters.join(",");
      router.push({ query: { type: args } });
    }
  }
});

watch(
  route,
  (newRoute) => {
    if (newRoute.query.type) {
      selectedFilters.value = newRoute.query.type
        .split(",")
        .filter((i) => ALL_FILTERS.indexOf(i) > -1);
    } else {
      selectedFilters.value = [...ALL_FILTERS];
    }
  },
  { immediate: true },
);
</script>

<style scoped>
.wrapper {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.filtersSection {
  display: flex;
  flex-direction: row;
  align-items: center;
  gap: 12px;
}

.filtersLabel {
  font-size: var(--text-sm);
  color: var(--text-subdued);
  white-space: nowrap;
}

.filtersContainer {
  display: flex;
  flex-direction: row;
  gap: 8px;
  flex-wrap: wrap;
}

.filter {
  padding: 8px 16px;
  border-radius: 4px;
  cursor: pointer;
  transition: scale 0.3s ease;
  cursor: pointer;
  font-weight: bold;
  transition:
    scale 0.3s ease,
    background-color 0.3s ease;
}

.filter:hover {
  transition:
    scale 0.3s ease,
    background-color 0.3s ease;
}

.filter:active {
  transition:
    scale 0.3s ease,
    background-color 0.3s ease;
}

.selectedFilter {
  background-color: var(--accent-color);
  color: white;
  transition:
    scale 0.3s ease,
    background-color 0.3s ease;
}

.searchResult {
  min-width: 300px;
}

.searchResultsContainer {
  flex: 1;
  display: grid;
  gap: 16px;
  grid-template-columns: repeat(1, 1fr);
  overflow-x: hidden;
  justify-items: start;
}

@media (min-width: 1200px) {
  .searchResultsContainer {
    grid-template-columns: repeat(2, 1fr);
  }
}

@media (min-width: 1600px) {
  .searchResultsContainer {
    grid-template-columns: repeat(3, 1fr);
  }
}

/* Results Section */
.resultsSection {
  margin-top: 0;
}

.sectionTitle {
  font-size: var(--text-lg);
  font-weight: var(--font-semibold);
  color: var(--text-base);
  margin: 0 0 var(--spacing-3) 0;
}

.noResults {
  color: var(--text-subdued);
  font-style: italic;
}
</style>
