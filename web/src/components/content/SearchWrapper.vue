<template>
  <div>
    <section v-if="query.trim()" class="workResults" :aria-busy="workLoading">
      <h2>Works</h2>
      <p v-if="workLoading" role="status">Searching works…</p>
      <p v-else-if="workError" role="status">Could not load matching works.</p>
      <p v-else-if="!works.length" role="status">No matching works.</p>
      <ul v-else>
        <li v-for="work in works" :key="work.id">
          <RouterLink :to="{ name: 'work', params: { workId: work.id } }">{{
            work.title
          }}</RouterLink>
          <span> — {{ work.creators.join(", ") }}</span>
        </li>
      </ul>
    </section>
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
import { ref, watch } from "vue";
import axios from "axios";

const props = defineProps({
  query: { type: String, default: "" },
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

const works = ref([]);
const workError = ref(false);
const workLoading = ref(false);
watch(
  () => props.query,
  async (query, _, onCleanup) => {
    works.value = [];
    workError.value = false;
    workLoading.value = false;
    if (!query.trim()) return;
    workLoading.value = true;
    const controller = new AbortController();
    onCleanup(() => controller.abort());
    try {
      const { data } = await axios.get("/v1/content/works", {
        params: { query, limit: 25 },
        signal: controller.signal,
      });
      if (!controller.signal.aborted) works.value = data;
    } catch {
      if (!controller.signal.aborted) workError.value = true;
    } finally {
      if (!controller.signal.aborted) workLoading.value = false;
    }
  },
  { immediate: true },
);
</script>

<style scoped>
.workResults {
  padding: 16px 24px;
}
.workResults li {
  margin: 12px 0;
}
.workResults a {
  color: inherit;
}
</style>
