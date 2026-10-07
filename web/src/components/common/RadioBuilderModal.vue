<template>
  <ModalDialog :isOpen="isOpen" :closeCallback="handleClose" :closeOnEsc="true">
    <div class="radioBuilder">
      <header class="builderHeader">
        <h2>Customize radio</h2>
        <button
          type="button"
          class="iconButton"
          aria-label="Close radio customization"
          @click="handleClose"
        >
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="m6 6 12 12M18 6 6 18" />
          </svg>
        </button>
      </header>

      <div v-if="isLoading" class="loadingState">Loading...</div>

      <div v-else class="builderBody">
        <section class="controlGroup">
          <label>
            <span>Recipe</span>
            <select
              v-model="selectedRecipeId"
              aria-label="Recipe"
              @change="applySelectedRecipe"
            >
              <option
                v-for="recipe in options?.recipes || []"
                :key="recipe.id"
                :value="recipe.id"
              >
                {{ recipe.name }}
              </option>
            </select>
          </label>

          <label>
            <span>Mode</span>
            <select v-model="mode" aria-label="Mode">
              <option value="similar">Similar</option>
              <option value="explore">Explore</option>
            </select>
          </label>

          <label>
            <span>Tracks</span>
            <input v-model.number="count" type="number" min="1" max="200" />
          </label>
        </section>

        <section class="controlGroup">
          <label>
            <span class="sliderLabel"
              >Diversity
              <output>{{ Math.round(diversity * 100) }}%</output></span
            >
            <input
              v-model.number="diversity"
              aria-label="Diversity"
              class="steerRange"
              :style="{ '--fill': `${diversity * 100}%` }"
              type="range"
              min="0"
              max="1"
              step="0.05"
            />
          </label>
          <label>
            <span class="sliderLabel"
              >Randomness
              <output>{{ Math.round(randomness * 100) }}%</output></span
            >
            <input
              v-model.number="randomness"
              aria-label="Randomness"
              class="steerRange"
              :style="{ '--fill': `${randomness * 100}%` }"
              type="range"
              min="0"
              max="1"
              step="0.05"
            />
          </label>
          <label class="checkRow">
            <input v-model="includeSeedTracks" type="checkbox" />
            <span>Include seed tracks</span>
          </label>
        </section>

        <section class="criteriaSection">
          <h3>Criteria</h3>
          <div
            v-for="criterion in criteria"
            :key="criterion.namespace"
            class="criterionRow"
          >
            <span>{{ criterionLabel(criterion.namespace) }}</span>
            <input
              v-model.number="criterion.weight"
              class="steerRange"
              :style="{ '--fill': `${criterion.weight * 100}%` }"
              :aria-label="`${criterionLabel(criterion.namespace)} importance`"
              type="range"
              min="0"
              max="1"
              step="0.05"
            />
            <strong>{{ Math.round(criterion.weight * 100) }}%</strong>
          </div>
        </section>

        <section class="referenceGrid">
          <ReferenceEditor
            title="Toward"
            v-model="toward"
            emptyText="Add influences to lean toward."
          />
          <ReferenceEditor
            title="Away"
            v-model="away"
            emptyText="Add influences to avoid."
          />
        </section>

        <section class="filtersSection">
          <h3>Filters</h3>
          <div class="filtersGrid">
            <label>
              <span>Genres</span>
              <input v-model="genres" type="text" />
            </label>
            <label>
              <span>Year from</span>
              <input v-model.number="releaseYearMin" type="number" min="0" />
            </label>
            <label>
              <span>Year to</span>
              <input v-model.number="releaseYearMax" type="number" min="0" />
            </label>
            <label>
              <span>Popularity from</span>
              <input
                v-model.number="popularityMin"
                type="number"
                min="0"
                max="100"
              />
            </label>
            <label>
              <span>Popularity to</span>
              <input
                v-model.number="popularityMax"
                type="number"
                min="0"
                max="100"
              />
            </label>
            <label>
              <span>Explicit</span>
              <select v-model="explicitFilter" aria-label="Explicit">
                <option value="include">Include</option>
                <option value="exclude">Exclude</option>
                <option value="only">Only</option>
              </select>
            </label>
          </div>
        </section>
      </div>

      <footer class="builderActions">
        <span v-if="validationError" role="alert">{{ validationError }}</span>
        <button class="cancelButton" @click="handleClose">Cancel</button>
        <button
          class="primaryButton"
          :disabled="isSubmitting || isLoading || !options"
          @click="handleSubmit"
        >
          {{ isSubmitting ? "Starting..." : "Start radio" }}
        </button>
      </footer>
    </div>
  </ModalDialog>
</template>

<script setup>
import { computed, ref, watch } from "vue";
import ModalDialog from "@/components/common/ModalDialog.vue";
import ReferenceEditor from "@/components/common/ReferenceEditor.vue";
import { usePlaybackStore } from "@/store/playback";
import { useRemoteStore } from "@/store/remote";

const props = defineProps({
  isOpen: {
    type: Boolean,
    required: true,
  },
  seedEntityType: {
    type: String,
    required: true,
  },
  seedEntityId: {
    type: String,
    required: true,
  },
});

const emit = defineEmits(["close"]);

const remoteStore = useRemoteStore();
const playback = usePlaybackStore();

const options = ref(null);
const isLoading = ref(false);
const isSubmitting = ref(false);
const selectedRecipeId = ref("balanced");
const mode = ref("similar");
const count = ref(50);
const diversity = ref(0.3);
const randomness = ref(0.3);
const includeSeedTracks = ref(props.seedEntityType !== "album");
const validationError = ref("");
const criteria = ref([]);
const toward = ref([]);
const away = ref([]);
const genres = ref("");
const releaseYearMin = ref(null);
const releaseYearMax = ref(null);
const popularityMin = ref(null);
const popularityMax = ref(null);
const explicitFilter = ref("include");

const criteriaByNamespace = computed(() => {
  const map = new Map();
  for (const criterion of options.value?.criteria || []) {
    map.set(criterion.namespace, criterion);
  }
  return map;
});

const criterionLabel = (namespace) =>
  criteriaByNamespace.value.get(namespace)?.label || namespace;

const handleClose = () => {
  if (isSubmitting.value) playback.cancelRadioCreation();
  emit("close");
};

const selectedRecipe = () =>
  (options.value?.recipes || []).find(
    (recipe) => recipe.id === selectedRecipeId.value,
  );

const applySelectedRecipe = () => {
  const recipe = selectedRecipe();
  if (!recipe) return;
  mode.value = recipe.mode || "similar";
  diversity.value = recipe.diversity ?? 0.3;
  randomness.value = recipe.randomness ?? 0.3;
  criteria.value = (recipe.criteria || []).map((criterion) => ({
    namespace: criterion.namespace,
    weight: criterion.weight,
  }));
};

const loadOptions = async () => {
  isLoading.value = true;
  options.value = await remoteStore.fetchRadioOptions();
  isLoading.value = false;
  if (!options.value)
    validationError.value =
      "Could not load radio options. Close and reopen to retry.";
  selectedRecipeId.value = options.value?.default_recipe_id || "balanced";
  count.value = options.value?.count?.default || 50;
  diversity.value = options.value?.diversity?.default ?? 0.3;
  randomness.value = options.value?.randomness?.default ?? 0.3;
  applySelectedRecipe();
};

const numberOrNull = (value) =>
  Number.isFinite(value) && value !== "" ? Number(value) : null;

const cleanedReferences = (items) =>
  items
    .filter((item) => item.entity_id && item.entity_id.trim().length > 0)
    .map((item) => ({
      entity_type: item.entity_type,
      entity_id: item.entity_id.trim(),
      weight: Number(item.weight) || 1,
    }));

const buildFilters = () => {
  const filters = {};
  const parsedGenres = genres.value
    .split(",")
    .map((genre) => genre.trim())
    .filter(Boolean);
  if (parsedGenres.length > 0) filters.genres = parsedGenres;
  const yearMin = numberOrNull(releaseYearMin.value);
  const yearMax = numberOrNull(releaseYearMax.value);
  const popMin = numberOrNull(popularityMin.value);
  const popMax = numberOrNull(popularityMax.value);
  if (yearMin !== null) filters.release_year_min = yearMin;
  if (yearMax !== null) filters.release_year_max = yearMax;
  if (popMin !== null) filters.popularity_min = popMin;
  if (popMax !== null) filters.popularity_max = popMax;
  if (explicitFilter.value !== "include")
    filters.explicit = explicitFilter.value;
  return Object.keys(filters).length > 0 ? filters : null;
};

const handleSubmit = async () => {
  validationError.value = "";
  if (!criteria.value.some((criterion) => criterion.weight > 0)) {
    validationError.value =
      "Choose at least one criterion with a positive weight.";
    return;
  }
  isSubmitting.value = true;
  const filters = buildFilters();
  const request = {
    count: count.value,
    recipe_id: selectedRecipeId.value,
    criteria: criteria.value.filter((criterion) => criterion.weight > 0),
    mode: mode.value,
    toward: cleanedReferences(toward.value),
    away: cleanedReferences(away.value),
    diversity: diversity.value,
    randomness: randomness.value,
    include_seed_tracks: includeSeedTracks.value,
  };
  if (filters) request.filters = filters;
  const trackIds = await playback.setAdvancedRadioFromItem(
    props.seedEntityType,
    props.seedEntityId,
    request,
  );
  isSubmitting.value = false;
  if (!trackIds.length)
    validationError.value =
      playback.radioCreationState.message || "Radio creation cancelled.";
  if (trackIds.length > 0) {
    handleClose();
  }
};

watch(
  () => props.isOpen,
  (isOpen) => {
    if (isOpen && !options.value) {
      loadOptions();
    }
  },
);
</script>

<style scoped>
@import "@/components/steering/steeringCard.css";
.radioBuilder {
  width: min(760px, calc(100vw - 80px));
  max-height: min(820px, calc(100dvh - 80px));
  display: flex;
  flex-direction: column;
  gap: 24px;
  color: var(--text-bright);
  color-scheme: dark;
}

.builderHeader,
.builderActions,
.referenceHeader {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.builderHeader h2,
.criteriaSection h3,
.referenceHeader h3 {
  margin: 0;
}

.builderBody {
  overflow: auto;
  display: flex;
  flex-direction: column;
  gap: 28px;
  min-height: 0;
  padding: 2px 4px;
  overscroll-behavior: contain;
}

.controlGroup,
.filtersGrid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 12px;
}

label,
.criterionRow,
.referenceRow {
  display: flex;
  align-items: center;
  gap: 8px;
}

label {
  flex-direction: column;
  align-items: stretch;
}

.checkRow {
  flex-direction: row;
  align-items: center;
  justify-content: flex-start;
}

input:not([type="range"]):not([type="checkbox"]),
select {
  width: 100%;
  border: 1px solid var(--border-default);
  border-radius: var(--radius-md);
  background: var(--bg-highlight);
  color: var(--text-bright);
  padding: 6px 10px;
  outline: none;
}

input:not([type="range"]):not([type="checkbox"])::placeholder {
  color: var(--text-subtle);
}

input:not([type="range"]):not([type="checkbox"]):focus,
select:focus {
  border-color: var(--spotify-green);
  box-shadow: 0 0 0 2px var(--bg-tinted);
}

input[type="range"],
input[type="checkbox"] {
  accent-color: var(--spotify-green);
}

label span,
.criterionRow span {
  color: var(--text-subdued);
}

button {
  border-radius: var(--radius-md);
  background: var(--bg-highlight);
  color: var(--text-bright);
  padding: 0 12px;
}

button:hover:not(:disabled) {
  background: var(--bg-press);
}

button:disabled {
  cursor: default;
  opacity: 0.6;
}

.criteriaSection,
.referenceSection {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.criterionRow span {
  width: 150px;
}

.criterionRow input {
  flex: 1;
}

.criterionRow strong {
  width: 44px;
  text-align: right;
}

.referenceGrid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
  gap: 16px;
}

.referenceRow {
  display: grid;
  grid-template-columns: 82px 1fr 70px 34px;
}

.iconButton {
  width: 34px;
  padding: 0;
}

.builderActions {
  justify-content: flex-end;
}

.primaryButton {
  background: var(--spotify-green);
  color: #000;
  font-weight: var(--font-semibold);
}

.primaryButton:hover:not(:disabled) {
  background: var(--spotify-green-hover);
}

.loadingState {
  min-height: 160px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.builderHeader {
  flex-shrink: 0;
}
.builderHeader h2 {
  font-size: 28px;
  font-weight: 750;
  letter-spacing: -0.025em;
}
h3 {
  font-size: 18px;
  font-weight: 700;
  margin: 0 0 14px;
}
label {
  font-size: 14px;
  gap: 10px;
}
label > span,
.criterionRow > span {
  color: var(--text-base);
}
input:not([type="range"]):not([type="checkbox"]),
select {
  min-height: 44px;
  padding: 10px 12px;
  border-radius: 4px;
  background: #333;
  border: 1px solid #727272;
  font: inherit;
}
input:not([type="range"]):not([type="checkbox"]):focus,
select:focus {
  border-color: #fff;
  box-shadow: inset 0 0 0 1px #fff;
}
input[type="range"] {
  min-width: 0;
}
input[type="checkbox"] {
  width: 18px;
  height: 18px;
  flex-shrink: 0;
}
.sliderLabel {
  display: flex;
  justify-content: space-between;
  gap: 12px;
}
output,
.criterionRow strong {
  color: var(--text-subdued);
  font-size: 13px;
  font-variant-numeric: tabular-nums;
  font-weight: 400;
}
.criteriaSection {
  gap: 18px;
}
.criteriaSection h3 {
  margin-bottom: 0;
}
.criterionRow {
  gap: 16px;
  font-size: 14px;
}
.criterionRow > span {
  width: 130px;
  flex-shrink: 0;
}
.referenceGrid,
.filtersSection {
  border-top: 1px solid var(--surface-border);
  padding-top: 24px;
}
.builderActions {
  flex-shrink: 0;
  padding-top: 20px;
  border-top: 1px solid var(--surface-border);
  flex-wrap: wrap;
}
.builderActions > span {
  color: var(--text-negative, #f3727f);
  font-size: 14px;
  flex: 1 1 180px;
}
.builderActions button {
  min-height: 48px;
  padding: 0 24px;
  border: 0;
  border-radius: 999px;
  font-weight: 700;
  cursor: pointer;
}
.builderActions .cancelButton {
  background: transparent;
  color: var(--text-subdued);
}
.builderActions .cancelButton:hover {
  color: var(--text-base);
}
.iconButton {
  display: grid;
  place-items: center;
  width: 36px;
  height: 36px;
  flex-shrink: 0;
  border: 0;
  border-radius: 50%;
  background: transparent;
  cursor: pointer;
}
.iconButton svg {
  width: 24px;
  height: 24px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.8;
}
button:focus-visible {
  outline: 2px solid #fff;
  outline-offset: 3px;
}
@media (max-width: 600px) {
  .builderHeader h2 {
    font-size: 24px;
  }
  .controlGroup,
  .filtersGrid,
  .referenceGrid {
    grid-template-columns: minmax(0, 1fr);
  }
  .criterionRow > span {
    width: 110px;
  }
  .criterionRow {
    gap: 10px;
  }
}
</style>
