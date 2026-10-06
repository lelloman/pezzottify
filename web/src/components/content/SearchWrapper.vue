<template>
  <div class="searchPage">
    <SearchOverview
      :items="overviewItems"
      :primary="primaryResult"
      :loading="isStreamingLoading"
    />
    <section
      v-if="query.trim() && (works.length || workLoading || workError)"
      class="workResults"
      :aria-busy="workLoading"
    >
      <h2 class="sectionTitle">Works</h2>
      <p v-if="workLoading" role="status">Searching works…</p>
      <p v-else-if="workError" role="status">Could not load matching works.</p>
      <p v-else-if="!works.length" role="status">No matching works.</p>
      <div v-else class="resultsGrid">
        <WorkResult
          v-for="work in visibleWorks"
          :key="work.id"
          :result="work"
        />
      </div>
      <button
        v-if="works.length > 6"
        type="button"
        class="showMore"
        :aria-expanded="expanded"
        @click="expanded = !expanded"
      >
        {{ expanded ? "Show fewer works" : `Show all ${works.length} works` }}
      </button>
    </section>
  </div>
</template>

<script setup>
import SearchOverview from "../search/SearchOverview.vue";
import WorkResult from "@/components/search/WorkResult.vue";
import { computed, ref, watch } from "vue";
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

const primaryResult = computed(() =>
  props.useOrganicSearch
    ? null
    : props.streamingSections.find(
        (section) => section.section.startsWith("primary_") && section.item,
      )?.item,
);
const overviewItems = computed(() => {
  const items = props.useOrganicSearch
    ? props.results || []
    : props.streamingSections.flatMap((section) => {
        if (section.item) return [section.item];
        const types = {
          popular_by: "Track",
          tracks_from: "Track",
          albums_by: "Album",
          related_artists: "Artist",
        };
        return (section.items || []).map((item) => ({
          ...item,
          type: item.type || types[section.section],
        }));
      });
  const seen = new Set();
  return [
    ...(primaryResult.value ? [primaryResult.value] : []),
    ...items,
  ].filter((item) => {
    if (!item.id || !["Album", "Artist", "Track"].includes(item.type))
      return false;
    const key = item.type + ":" + item.id;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
});

const works = ref([]);
const expanded = ref(false);
const visibleWorks = computed(() =>
  expanded.value ? works.value : works.value.slice(0, 6),
);
const workError = ref(false);
const workLoading = ref(false);
watch(
  () => props.query,
  async (query, _, onCleanup) => {
    works.value = [];
    expanded.value = false;
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
.searchPage {
  padding: 24px;
  min-width: 0;
}
.workResults {
  margin-top: 28px;
}
.workResults {
  margin-bottom: 24px;
}
.sectionTitle {
  font-size: 24px;
  font-weight: var(--font-semibold);
  color: var(--text-base);
  margin: 0 0 12px;
}
.resultsGrid {
  display: grid;
  gap: 16px;
  grid-template-columns: minmax(0, 1fr);
}
@media (min-width: 1200px) {
  .resultsGrid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
@media (min-width: 1600px) {
  .resultsGrid {
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }
}
.showMore {
  margin: 12px 8px 0;
  padding: 8px 12px;
  background: var(--surface-raised);
  border: 1px solid var(--surface-border);
  border-radius: 4px;
  color: var(--text-base);
  font: inherit;
  cursor: pointer;
}
.showMore:hover {
  background: var(--surface-hover);
}
.showMore:focus-visible {
  outline: 2px solid var(--accent-color);
  outline-offset: 2px;
}
</style>
