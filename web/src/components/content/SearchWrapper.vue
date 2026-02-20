<template>
  <div>
    <SearchResults v-if="useOrganicSearch" :results="results" />
    <StreamingSearchResults
      v-else
      :sections="streamingSections"
      :isLoading="isStreamingLoading"
    />
  </div>
</template>

<script setup>
import SearchResults from "./SearchResults.vue";
import StreamingSearchResults from "./StreamingSearchResults.vue";

defineProps({
  useOrganicSearch: Boolean,
  results: {
    type: Array,
    default: () => [],
  },
  streamingSections: {
    type: Array,
    default: () => [],
  },
  isStreamingLoading: Boolean,
});
</script>
