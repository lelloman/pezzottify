<template>
  <div class="referencePicker">
    <div v-if="allowConcepts" class="segmented" role="tablist">
      <button
        type="button"
        role="tab"
        :aria-selected="tab === 'catalog'"
        :class="{ active: tab === 'catalog' }"
        @click="tab = 'catalog'"
      >
        Search
      </button>
      <button
        type="button"
        role="tab"
        data-test="concepts-tab"
        :aria-selected="tab === 'concepts'"
        :class="{ active: tab === 'concepts' }"
        @click="tab = 'concepts'"
      >
        Browse concepts
      </button>
    </div>

    <template v-if="tab === 'concepts'">
      <label class="searchField">
        <span class="searchIcon" aria-hidden="true" />
        <input
          v-model="conceptQuery"
          type="search"
          placeholder="Genres, instruments, moods, decades"
          aria-label="Filter concepts"
          @keydown.esc="conceptQuery = ''"
        />
      </label>
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
          <div class="conceptGrid">
            <button
              v-for="concept in group.items"
              :key="concept.id"
              type="button"
              class="conceptTile"
              :style="{ backgroundColor: tileColor(concept.id) }"
              :title="`${concept.example_count} example tracks`"
              @click="selectConcept(concept)"
            >
              <span class="conceptTileLabel">{{ concept.label }}</span>
              <span class="conceptTileShape" aria-hidden="true" />
            </button>
          </div>
        </section>
      </div>
    </template>

    <template v-else>
      <label class="searchField">
        <span class="searchIcon" aria-hidden="true" />
        <input
          v-model="query"
          type="search"
          :placeholder="placeholder"
          :aria-label="placeholder"
          @keydown.esc="clear"
        />
      </label>
      <p v-if="tooShort" class="pickerHint">Type at least 2 characters</p>
      <p v-else-if="isLoading && !results.length" class="pickerHint">
        Searching…
      </p>
      <p v-else-if="error" class="pickerHint" role="alert">{{ error }}</p>
      <p
        v-else-if="searched && !isLoading && !results.length"
        class="pickerHint"
      >
        No artists, albums or tracks found.
      </p>
      <ul v-if="results.length" class="pickerResults">
        <li
          v-for="result in results"
          :key="result.entity_type + result.entity_id"
        >
          <button type="button" class="resultRow" @click="select(result)">
            <SteeringArtwork :reference="result" size="sm" />
            <span class="resultText">
              <span class="resultName">{{ result.label }}</span>
              <span class="resultDetail">
                {{ typeLabel(result.entity_type)
                }}<template v-if="result.detail">
                  · {{ result.detail }}</template
                >
              </span>
            </span>
          </button>
        </li>
      </ul>
    </template>
  </div>
</template>

<script setup>
import { computed, onUnmounted, ref, watch } from "vue";
import { debounce } from "lodash-es";
import { streamingSearch } from "@/services/streamingSearch";
import {
  canSearch,
  sectionsToReferences,
  SEARCH_DEBOUNCE_MS,
} from "@/utils/steeringSearch";
import { useRemoteStore } from "@/store/remote";
import { groupConcepts, matchesConceptQuery } from "@/utils/concepts";
import { tileColor } from "@/utils/steeringArt";
import SteeringArtwork from "./SteeringArtwork.vue";

const TYPE_LABELS = { artist: "Artist", album: "Album", track: "Song" };
const typeLabel = (type) => TYPE_LABELS[type] || type;

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

const query = ref("");
const results = ref([]);
const isLoading = ref(false);
const searched = ref(false);
const error = ref("");
const tooShort = computed(() => {
  const trimmed = query.value.trim();
  return trimmed.length > 0 && !canSearch(trimmed);
});

// Abort function of the running stream. Closing it drops the connection, so the
// server stops streaming sections for a query nobody will read.
let abortStream = null;
const stopStream = () => {
  abortStream?.();
  abortStream = null;
};

// Same streaming search as the main search screen; references update as each
// section arrives, primary match first.
const search = debounce((text) => {
  stopStream();
  const sections = [];
  isLoading.value = true;
  searched.value = false;
  error.value = "";
  results.value = [];
  let abort = null;
  const isCurrent = () => abortStream === abort;
  abort = streamingSearch(
    text,
    (section) => {
      if (!isCurrent()) return;
      sections.push(section);
      results.value = sectionsToReferences(sections);
    },
    (err) => {
      if (!isCurrent()) return;
      console.error("Steering reference search failed:", err);
      error.value = "Search failed. Try again.";
      results.value = [];
      isLoading.value = false;
      searched.value = true;
      abortStream = null;
    },
    () => {
      if (!isCurrent()) return;
      isLoading.value = false;
      searched.value = true;
      abortStream = null;
    },
  );
  abortStream = abort;
}, SEARCH_DEBOUNCE_MS);

watch(query, (text) => {
  const trimmed = text.trim();
  search.cancel();
  stopStream();
  error.value = "";
  if (!canSearch(trimmed)) {
    results.value = [];
    isLoading.value = false;
    searched.value = false;
    return;
  }
  isLoading.value = true;
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
  stopStream();
});
</script>

<style scoped>
.referencePicker {
  display: flex;
  flex-direction: column;
  gap: 14px;
  min-width: 0;
}

.segmented {
  display: inline-flex;
  align-self: flex-start;
  gap: 8px;
}

.segmented button {
  min-height: 32px;
  padding: 0 14px;
  border: none;
  border-radius: var(--radius-full);
  background: rgba(255, 255, 255, 0.07);
  color: var(--text-bright);
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
  cursor: pointer;
  transition: background-color var(--transition-fast);
}

.segmented button:hover {
  background: rgba(255, 255, 255, 0.12);
}

.segmented button.active {
  background: var(--text-bright);
  color: #000;
}

.searchField {
  position: relative;
  display: flex;
  align-items: center;
}

.searchIcon {
  position: absolute;
  left: 14px;
  width: 14px;
  height: 14px;
  border: 2px solid var(--text-subdued);
  border-radius: 50%;
  pointer-events: none;
}

.searchIcon::after {
  content: "";
  position: absolute;
  right: -5px;
  bottom: -4px;
  width: 6px;
  height: 2px;
  border-radius: 2px;
  background: var(--text-subdued);
  transform: rotate(45deg);
}

.searchField input {
  width: 100%;
  min-height: 44px;
  padding: 0 16px 0 40px;
  border: 2px solid transparent;
  border-radius: var(--radius-full);
  background: rgba(255, 255, 255, 0.08);
  color: var(--text-bright);
  font-size: var(--text-md);
  transition:
    background-color var(--transition-fast),
    border-color var(--transition-fast);
}

.searchField input:hover {
  background: rgba(255, 255, 255, 0.11);
}

.searchField input:focus {
  border-color: var(--text-bright);
  outline: none;
}

.pickerHint {
  margin: 0;
  color: var(--text-subdued);
  font-size: var(--text-sm);
}

.pickerResults {
  display: flex;
  flex-direction: column;
  margin: 0;
  padding: 0;
  list-style: none;
}

.resultRow {
  display: flex;
  align-items: center;
  gap: 12px;
  width: 100%;
  padding: 8px;
  border: none;
  border-radius: var(--radius-md);
  background: transparent;
  color: inherit;
  text-align: left;
  cursor: pointer;
}

.resultRow:hover {
  background: rgba(255, 255, 255, 0.08);
}

.resultRow :deep(.steeringArtwork) {
  box-shadow: none;
}

.resultText {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.resultName {
  overflow: hidden;
  color: var(--text-bright);
  font-size: var(--text-md);
  font-weight: var(--font-semibold);
  white-space: nowrap;
  text-overflow: ellipsis;
}

.resultDetail {
  overflow: hidden;
  color: var(--text-subdued);
  font-size: var(--text-sm);
  white-space: nowrap;
  text-overflow: ellipsis;
}

.conceptGroups {
  display: flex;
  flex-direction: column;
  gap: 22px;
  max-height: 560px;
  overflow-y: auto;
  padding-right: 4px;
}

.conceptFamily {
  margin: 0 0 12px;
  color: var(--text-bright);
  font-size: var(--text-lg);
  font-weight: var(--font-bold);
}

.conceptGrid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
  gap: 12px;
}

.conceptTile {
  position: relative;
  display: flex;
  align-items: flex-start;
  aspect-ratio: 16 / 10;
  overflow: hidden;
  padding: 12px;
  border: none;
  border-radius: var(--radius-lg);
  color: #fff;
  text-align: left;
  cursor: pointer;
  transition:
    transform var(--transition-fast),
    filter var(--transition-fast);
}

.conceptTile:hover {
  filter: brightness(1.12);
  transform: scale(1.02);
}

.conceptTileLabel {
  position: relative;
  z-index: 1;
  display: block;
  font-size: var(--text-lg);
  font-weight: var(--font-black);
  line-height: 1.15;
  letter-spacing: -0.01em;
  overflow-wrap: anywhere;
}

.conceptTileShape {
  position: absolute;
  right: -14px;
  bottom: -12px;
  width: 64px;
  height: 64px;
  border-radius: var(--radius-md);
  background: rgba(0, 0, 0, 0.22);
  box-shadow: -6px 6px 18px rgba(0, 0, 0, 0.25);
  transform: rotate(25deg);
}

@media (max-width: 600px) {
  .conceptGrid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
  }

  .conceptTileLabel {
    font-size: var(--text-md);
  }
}
</style>
