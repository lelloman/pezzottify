<template>
  <ModalDialog
    :isOpen="isOpen"
    :closeCallback="handleClose"
    :closeOnEsc="true"
  >
    <div class="radioBuilder">
      <header class="builderHeader">
        <h2>Customize radio</h2>
        <button class="iconButton" @click="handleClose">x</button>
      </header>

      <div v-if="isLoading" class="loadingState">Loading...</div>

      <div v-else class="builderBody">
        <section class="controlGroup">
          <label>
            <span>Recipe</span>
            <select v-model="selectedRecipeId" @change="applySelectedRecipe">
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
            <select v-model="mode">
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
            <span>Diversity</span>
            <input v-model.number="diversity" type="range" min="0" max="1" step="0.05" />
          </label>
          <label>
            <span>Randomness</span>
            <input v-model.number="randomness" type="range" min="0" max="1" step="0.05" />
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
            <input v-model.number="criterion.weight" type="range" min="0" max="1" step="0.05" />
            <strong>{{ criterion.weight.toFixed(2) }}</strong>
          </div>
        </section>

        <section class="referenceGrid">
          <ReferenceEditor title="Toward" v-model="toward" />
          <ReferenceEditor title="Away" v-model="away" />
        </section>

        <section class="filtersGrid">
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
            <input v-model.number="popularityMin" type="number" min="0" max="100" />
          </label>
          <label>
            <span>Popularity to</span>
            <input v-model.number="popularityMax" type="number" min="0" max="100" />
          </label>
          <label>
            <span>Explicit</span>
            <select v-model="explicitFilter">
              <option value="include">Include</option>
              <option value="exclude">Exclude</option>
              <option value="only">Only</option>
            </select>
          </label>
        </section>
      </div>

      <footer class="builderActions">
        <button @click="handleClose">Cancel</button>
        <button class="primaryButton" :disabled="isSubmitting" @click="handleSubmit">
          {{ isSubmitting ? "Starting..." : "Start radio" }}
        </button>
      </footer>
    </div>
  </ModalDialog>
</template>

<script setup>
import { computed, defineComponent, h, ref, watch } from "vue";
import ModalDialog from "@/components/common/ModalDialog.vue";
import { usePlaybackStore } from "@/store/playback";
import { useRemoteStore } from "@/store/remote";

const ReferenceEditor = defineComponent({
  props: {
    title: {
      type: String,
      required: true,
    },
    modelValue: {
      type: Array,
      required: true,
    },
  },
  emits: ["update:modelValue"],
  setup(props, { emit }) {
    const addReference = () => {
      emit("update:modelValue", [
        ...props.modelValue,
        { entity_type: "track", entity_id: "", weight: 1 },
      ]);
    };
    const updateReference = (index, patch) => {
      const next = props.modelValue.map((reference, itemIndex) =>
        itemIndex === index ? { ...reference, ...patch } : reference,
      );
      emit("update:modelValue", next);
    };
    const removeReference = (index) => {
      emit(
        "update:modelValue",
        props.modelValue.filter((_, itemIndex) => itemIndex !== index),
      );
    };

    return () =>
      h("section", { class: "referenceSection" }, [
        h("div", { class: "referenceHeader" }, [
          h("h3", props.title),
          h("button", { onClick: addReference }, "+"),
        ]),
        ...props.modelValue.map((reference, index) =>
          h("div", { class: "referenceRow" }, [
            h(
              "select",
              {
                value: reference.entity_type,
                onChange: (event) =>
                  updateReference(index, { entity_type: event.target.value }),
              },
              ["track", "album", "artist"].map((type) =>
                h("option", { value: type }, type),
              ),
            ),
            h("input", {
              value: reference.entity_id,
              placeholder: "ID",
              onInput: (event) =>
                updateReference(index, { entity_id: event.target.value }),
            }),
            h("input", {
              type: "number",
              min: "0",
              step: "0.25",
              value: reference.weight,
              onInput: (event) =>
                updateReference(index, {
                  weight: Number(event.target.value) || 1,
                }),
            }),
            h("button", { onClick: () => removeReference(index) }, "x"),
          ]),
        ),
      ]);
  },
});

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
const includeSeedTracks = ref(true);
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
  if (explicitFilter.value !== "include") filters.explicit = explicitFilter.value;
  return Object.keys(filters).length > 0 ? filters : null;
};

const handleSubmit = async () => {
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
.radioBuilder {
  width: min(760px, 88vw);
  max-height: 84vh;
  display: flex;
  flex-direction: column;
  gap: 16px;
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
  gap: 16px;
}

.controlGroup,
.filtersGrid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
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

input,
select,
button {
  min-height: 34px;
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
  background: var(--accent-color);
  color: var(--text-bright);
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
</style>
