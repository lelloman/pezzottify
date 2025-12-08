<template>
  <div class="wrapper">
    <div class="filtersContainer">
      <div
        :class="{
          filter: true,
          selectedFilter: selectedFilters.indexOf('album') > -1,
          scaleClickFeedback: true,
        }"
        @click.stop="toggleFilter('album')"
      >
        Albums
      </div>
      <div
        :class="{
          filter: true,
          selectedFilter: selectedFilters.indexOf('artist') > -1,
          scaleClickFeedback: true,
        }"
        @click.stop="toggleFilter('artist')"
      >
        Artists
      </div>
      <div
        :class="{
          filter: true,
          selectedFilter: selectedFilters.indexOf('track') > -1,
          scaleClickFeedback: true,
        }"
        @click.stop="toggleFilter('track')"
      >
        Tracks
      </div>
    </div>
    <div class="searchResultsContainer">
      <div v-for="(result, index) in results" :key="index" class="searchResult">
        <AlbumResult v-if="result.type === 'Album'" :result="result" />
        <ArtistResult v-else-if="result.type === 'Artist'" :result="result" />
        <TrackResult v-else-if="result.type === 'Track'" :result="result" />
        <div v-else class="">
          <p>Unknown result type</p>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, watch, defineProps } from "vue";
import AlbumResult from "@/components/search/AlbumResult.vue";
import ArtistResult from "@/components/search/ArtistResult.vue";
import TrackResult from "@/components/search/TrackResult.vue";
import { useRoute, useRouter } from "vue-router";

const props = defineProps({
  results: {
    type: Array,
    required: true,
  },
});

const selectedFilters = ref(["album", "artist", "track"]);
const isLoading = ref(true);

const router = useRouter();
const route = useRoute();

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
    if (newFilters.length == 3) {
      // remove query parameters
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
    const possibleValues = ["album", "artist", "track"];
    if (newRoute.query.type) {
      selectedFilters.value = newRoute.query.type
        .split(",")
        .filter((i) => possibleValues.indexOf(i) > -1);
    } else {
      selectedFilters.value = possibleValues;
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

.filtersContainer {
  display: flex;
  flex-direction: row;
  gap: 16px;
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
</style>
