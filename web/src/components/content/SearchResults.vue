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

    <!-- Catalog Results Section -->
    <div class="resultsSection">
      <h2 class="sectionTitle">Catalog Results</h2>
      <div v-if="results && results.length > 0" class="searchResultsContainer">
        <div v-for="(result, index) in results" :key="index" class="searchResult">
          <AlbumResult v-if="result.type === 'Album'" :result="result" />
          <ArtistResult v-else-if="result.type === 'Artist'" :result="result" />
          <TrackResult v-else-if="result.type === 'Track'" :result="result" />
        </div>
      </div>
      <p v-else class="noResults">No results found in catalog</p>
    </div>

    <!-- External Results Section -->
    <div v-if="showExternalSearch" class="resultsSection externalSection">
      <div class="sectionHeader">
        <h2 class="sectionTitle">External Results</h2>
        <div v-if="externalLimits" class="limitsInfo">
          <span class="limitBadge" :class="{ limitWarning: !externalLimits.can_request }">
            {{ externalLimits.requests_today }}/{{ externalLimits.max_per_day }} today
          </span>
          <span class="limitBadge" :class="{ limitWarning: externalLimits.in_queue >= externalLimits.max_queue }">
            {{ externalLimits.in_queue }}/{{ externalLimits.max_queue }} in queue
          </span>
        </div>
      </div>
      <div v-if="externalResults && externalResults.results && externalResults.results.length > 0" class="searchResultsContainer">
        <div v-for="result in externalResults.results" :key="result.id" class="externalResult">
          <div class="externalResultCard">
            <img
              v-if="result.image_url"
              :src="result.image_url"
              :alt="result.name"
              class="externalResultImage"
            />
            <div v-else class="externalResultImagePlaceholder"></div>
            <div class="externalResultInfo">
              <span class="externalResultName">{{ result.name }}</span>
              <span v-if="result.artist_name" class="externalResultArtist">
                {{ result.artist_name }}
              </span>
              <span v-if="result.year" class="externalResultYear">{{ result.year }}</span>
            </div>
            <div class="externalResultActions">
              <span v-if="result.in_catalog" class="statusBadge inCatalog">In Catalog</span>
              <span v-else-if="result.in_queue" class="statusBadge inQueue">In Queue</span>
              <button
                v-else
                class="requestButton scaleClickFeedback"
                :disabled="!externalLimits || !externalLimits.can_request"
                @click="$emit('request-album', result)"
              >
                Request
              </button>
            </div>
          </div>
        </div>
      </div>
      <p v-else-if="externalResults" class="noResults">No external results found</p>
      <p v-else class="noResults loadingText">Searching external providers...</p>
    </div>
  </div>
</template>

<script setup>
import { ref, watch, computed, defineProps, defineEmits } from "vue";
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
  externalResults: {
    type: Object,
    default: null,
  },
  externalLimits: {
    type: Object,
    default: null,
  },
  showExternalSearch: {
    type: Boolean,
    default: false,
  },
});

defineEmits(["request-album"]);

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
  margin-top: var(--spacing-4);
}

.sectionHeader {
  display: flex;
  justify-content: space-between;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--spacing-2);
  margin-bottom: var(--spacing-3);
}

.sectionTitle {
  font-size: var(--text-lg);
  font-weight: var(--font-semibold);
  color: var(--text-base);
  margin: 0;
}

.noResults {
  color: var(--text-subdued);
  font-style: italic;
}

.loadingText {
  animation: pulse 1.5s ease-in-out infinite;
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.5; }
}

/* External Section */
.externalSection {
  margin-top: var(--spacing-6);
  padding-top: var(--spacing-4);
  border-top: 1px solid var(--border-subdued);
}

.limitsInfo {
  display: flex;
  gap: var(--spacing-2);
}

.limitBadge {
  font-size: var(--text-xs);
  padding: 4px 8px;
  border-radius: var(--radius-sm);
  background-color: var(--bg-elevated);
  color: var(--text-subdued);
}

.limitBadge.limitWarning {
  background-color: rgba(239, 68, 68, 0.2);
  color: #ef4444;
}

/* External Result Card */
.externalResult {
  min-width: 300px;
}

.externalResultCard {
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
  padding: var(--spacing-3);
  background-color: var(--bg-elevated);
  border-radius: var(--radius-md);
  transition: background-color var(--transition-fast);
}

.externalResultCard:hover {
  background-color: var(--bg-highlight);
}

.externalResultImage {
  width: 56px;
  height: 56px;
  border-radius: var(--radius-sm);
  object-fit: cover;
  flex-shrink: 0;
}

.externalResultImagePlaceholder {
  width: 56px;
  height: 56px;
  border-radius: var(--radius-sm);
  background-color: var(--bg-subdued);
  flex-shrink: 0;
}

.externalResultInfo {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.externalResultName {
  font-weight: var(--font-medium);
  color: var(--text-base);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.externalResultArtist {
  font-size: var(--text-sm);
  color: var(--text-subdued);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.externalResultYear {
  font-size: var(--text-xs);
  color: var(--text-subdued);
}

.externalResultActions {
  flex-shrink: 0;
}

.statusBadge {
  font-size: var(--text-xs);
  padding: 4px 10px;
  border-radius: var(--radius-full);
  font-weight: var(--font-medium);
}

.statusBadge.inCatalog {
  background-color: rgba(34, 197, 94, 0.2);
  color: #22c55e;
}

.statusBadge.inQueue {
  background-color: rgba(249, 115, 22, 0.2);
  color: #f97316;
}

.requestButton {
  padding: 6px 14px;
  border-radius: var(--radius-full);
  border: none;
  background-color: var(--spotify-green);
  color: white;
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  cursor: pointer;
  transition: opacity var(--transition-fast);
}

.requestButton:hover:not(:disabled) {
  opacity: 0.9;
}

.requestButton:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
</style>
