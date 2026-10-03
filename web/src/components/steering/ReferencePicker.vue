<template>
  <div class="referencePicker">
    <div v-if="allowConcepts" class="pickerTabs" role="tablist">
      <button
        type="button"
        role="tab"
        :aria-selected="tab === 'catalog'"
        :class="{ active: tab === 'catalog' }"
        @click="tab = 'catalog'"
      >
        Artists, albums, tracks
      </button>
      <button
        type="button"
        role="tab"
        :aria-selected="tab === 'concepts'"
        :class="{ active: tab === 'concepts' }"
        @click="tab = 'concepts'"
      >
        Concepts
      </button>
    </div>

    <template v-if="tab === 'concepts'">
      <input
        v-model="conceptQuery"
        type="search"
        placeholder="Filter genres, instruments, moods, decades"
        aria-label="Filter concepts"
        @keydown.esc="conceptQuery = ''"
      />
      <p v-if="conceptsLoading" class="pickerHint">Loading concepts…</p>
      <p v-else-if="conceptsError" class="pickerHint" role="alert">
        {{ conceptsError }}
      </p>
      <p v-else-if="!conceptGroups.length" class="pickerHint">
        No concepts match.
      </p>
      <div v-else class="conceptGroups">
        <section v-for="group in conceptGroups" :key="group.family">
          <h4 class="conceptFamily">{{ group.label }}</h4>
          <div class="conceptChips">
            <button
              v-for="concept in group.items"
              :key="concept.id"
              type="button"
              class="conceptChip"
              :title="`${concept.example_count} example tracks`"
              @click="selectConcept(concept)"
            >
              {{ concept.label }}
            </button>
          </div>
        </section>
      </div>
    </template>

    <template v-else>
      <input
        v-model="query"
        type="search"
        :placeholder="placeholder"
        :aria-label="placeholder"
        @keydown.esc="clear"
      />
      <p v-if="isLoading" class="pickerHint">Searching…</p>
      <p v-else-if="error" class="pickerHint" role="alert">{{ error }}</p>
      <p v-else-if="query.trim() && !results.length" class="pickerHint">
        No artists, albums or tracks found.
      </p>
      <ul v-if="results.length" class="pickerResults">
        <li
          v-for="result in results"
          :key="result.entity_type + result.entity_id"
        >
          <button type="button" @click="select(result)">
            <span class="resultType">{{ result.entity_type }}</span>
            <span class="resultName">{{ result.label }}</span>
            <span v-if="result.detail" class="resultDetail">
              {{ result.detail }}
            </span>
          </button>
        </li>
      </ul>
    </template>
  </div>
</template>

<script setup>
import { computed, onUnmounted, ref, watch } from "vue";
import axios from "axios";
import { debounce } from "lodash-es";
import { useRemoteStore } from "@/store/remote";
import { groupConcepts, matchesConceptQuery } from "@/utils/concepts";

const props = defineProps({
  placeholder: {
    type: String,
    default: "Search an artist, album or track",
  },
  // Concepts can be destination/source components; radio references cannot.
  allowConcepts: {
    type: Boolean,
    default: true,
  },
});

const emit = defineEmits(["select"]);

const remote = useRemoteStore();
const tab = ref("catalog");
const conceptQuery = ref("");
const conceptsLoading = ref(false);
const conceptsError = ref("");

// The concept list is small (a few hundred entries) and changes weekly, so it is
// fetched once per page load and filtered locally.
let conceptCache = null;
const concepts = ref([]);

const loadConcepts = async () => {
  if (conceptCache) {
    concepts.value = conceptCache;
    return;
  }
  conceptsLoading.value = true;
  conceptsError.value = "";
  try {
    conceptCache = await remote.fetchConcepts({ limit: 500 });
    concepts.value = conceptCache;
  } catch (err) {
    console.error("Failed to load steering concepts:", err);
    conceptsError.value = "Could not load concepts. Try again.";
  } finally {
    conceptsLoading.value = false;
  }
};

watch(tab, (value) => {
  if (value === "concepts" && props.allowConcepts) loadConcepts();
});

const conceptGroups = computed(() =>
  groupConcepts(
    concepts.value.filter((concept) =>
      matchesConceptQuery(concept, conceptQuery.value),
    ),
  ),
);

const selectConcept = (concept) => {
  emit("select", {
    entity_type: "concept",
    entity_id: concept.id,
    label: concept.label,
    family: concept.family,
  });
};

const SEARCH_LIMIT = 12;
const TYPES = { Artist: "artist", Album: "album", Track: "track" };

const query = ref("");
const results = ref([]);
const isLoading = ref(false);
const error = ref("");
let abortController = null;

const artistNames = (result) =>
  (result.artists_ids_names || [])
    .map((entry) => (Array.isArray(entry) ? entry[1] : entry?.name))
    .filter(Boolean)
    .join(", ");

// Only artists, albums and tracks can be steering references.
const toReference = (result) => {
  const entityType = TYPES[result.type];
  if (!entityType || !result.id) return null;
  return {
    entity_type: entityType,
    entity_id: result.id,
    label: result.name || result.id,
    detail: entityType === "artist" ? "" : artistNames(result),
  };
};

const search = debounce(async (text) => {
  abortController?.abort();
  abortController = new AbortController();
  isLoading.value = true;
  error.value = "";
  try {
    const response = await axios.post(
      "/v1/content/search",
      {
        query: text,
        resolve: true,
        limit: SEARCH_LIMIT,
        search_mode: "expanded",
      },
      { signal: abortController.signal, timeout: 20000 },
    );
    const payload = Array.isArray(response.data) ? response.data : [];
    results.value = payload.map(toReference).filter(Boolean);
  } catch (err) {
    if (axios.isCancel?.(err) || err?.name === "CanceledError") return;
    console.error("Steering reference search failed:", err);
    error.value = "Search failed. Try again.";
    results.value = [];
  } finally {
    isLoading.value = false;
  }
}, 350);

watch(query, (text) => {
  const trimmed = text.trim();
  if (!trimmed) {
    search.cancel();
    abortController?.abort();
    results.value = [];
    isLoading.value = false;
    error.value = "";
    return;
  }
  search(trimmed);
});

const clear = () => {
  query.value = "";
};

const select = (result) => {
  emit("select", {
    entity_type: result.entity_type,
    entity_id: result.entity_id,
    label: result.label,
  });
  clear();
};

onUnmounted(() => {
  search.cancel();
  abortController?.abort();
});
</script>

<style scoped>
.pickerTabs {
  display: flex;
  gap: 4px;
}

.pickerTabs button {
  min-height: 30px;
  padding: 0 10px;
  border-radius: 999px;
  background: var(--bg-highlight);
  color: var(--text-subdued);
  font-size: 0.82rem;
}

.pickerTabs button.active {
  background: var(--bg-press);
  color: var(--text-bright);
}

.conceptGroups {
  display: flex;
  flex-direction: column;
  gap: 10px;
  max-height: 320px;
  overflow: auto;
}

.conceptFamily {
  margin: 0 0 6px;
  color: var(--text-subdued);
  font-size: 0.75rem;
  font-weight: 800;
  letter-spacing: 0.04em;
  text-transform: uppercase;
}

.conceptChips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.conceptChip {
  min-height: 28px;
  padding: 0 10px;
  border: 1px solid var(--border-default);
  border-radius: 999px;
  background: transparent;
  color: var(--text-bright);
  font-size: 0.85rem;
}

.conceptChip:hover,
.conceptChip:focus-visible {
  border-color: var(--spotify-green);
  background: var(--bg-highlight);
}

.referencePicker {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

input {
  min-height: 36px;
  border: 1px solid var(--border-default);
  border-radius: var(--radius-md);
  background: var(--bg-highlight);
  color: var(--text-bright);
  padding: 6px 10px;
  outline: none;
}

input:focus {
  border-color: var(--spotify-green);
  box-shadow: 0 0 0 2px var(--bg-tinted);
}

.pickerHint {
  margin: 0;
  color: var(--text-subdued);
  font-size: 0.85rem;
}

.pickerResults {
  display: flex;
  flex-direction: column;
  gap: 2px;
  max-height: 260px;
  overflow: auto;
  margin: 0;
  padding: 0;
  list-style: none;
}

.pickerResults button {
  width: 100%;
  display: grid;
  grid-template-columns: 56px minmax(0, 1fr);
  align-items: baseline;
  column-gap: 10px;
  padding: 8px 10px;
  border-radius: var(--radius-md);
  background: transparent;
  color: var(--text-bright);
  text-align: left;
}

.pickerResults button:hover,
.pickerResults button:focus-visible {
  background: var(--surface-hover, var(--bg-highlight));
}

.resultType {
  grid-row: span 2;
  color: var(--text-subdued);
  font-size: 0.75rem;
  text-transform: uppercase;
}

.resultName,
.resultDetail {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.resultDetail {
  color: var(--text-subdued);
  font-size: 0.8rem;
}
</style>
