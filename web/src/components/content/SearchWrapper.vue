<template>
  <div>
    <section v-if="works.length || workError" class="workResults">
      <h2>Works</h2>
      <p v-if="workError" role="status">Could not load matching works.</p>
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
watch(
  () => props.query,
  async (query, _, onCleanup) => {
    works.value = [];
    workError.value = false;
    if (!query.trim()) return;
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
