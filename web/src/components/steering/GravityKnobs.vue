<template>
  <section class="steeringCard">
    <header class="cardHeader">
      <span class="cardTitle">Knobs</span>
      <button
        type="button"
        class="textButton"
        :disabled="!hasCustomKnobs"
        @click="resetKnobs"
      >
        Reset to defaults
      </button>
    </header>

    <div class="knobGrid">
      <label class="knob">
        <span class="knobLabel">
          Recency
          <em v-if="knobs.recency_weight == null">default</em>
        </span>
        <input
          type="range"
          min="0"
          max="1"
          step="0.05"
          :value="displayed.recency_weight"
          @input="draft.recency_weight = Number($event.target.value)"
          @change="commit({ recency_weight: draft.recency_weight })"
        />
        <span class="knobHint">
          {{ format(displayed.recency_weight) }}: pull from the last tracks
          played versus the source
        </span>
      </label>

      <label class="knob">
        <span class="knobLabel">
          Diversity
          <em v-if="knobs.diversity == null">default</em>
        </span>
        <input
          type="range"
          min="0"
          max="1"
          step="0.05"
          :value="displayed.diversity"
          @input="draft.diversity = Number($event.target.value)"
          @change="commit({ diversity: draft.diversity })"
        />
        <span class="knobHint">
          {{ format(displayed.diversity) }}: penalty for repeating artists and
          albums
        </span>
      </label>

      <label class="knob">
        <span class="knobLabel">
          Randomness
          <em v-if="knobs.randomness == null">default</em>
        </span>
        <input
          type="range"
          min="0"
          max="1"
          step="0.05"
          :value="displayed.randomness"
          @input="draft.randomness = Number($event.target.value)"
          @change="commit({ randomness: draft.randomness })"
        />
        <span class="knobHint">
          {{ format(displayed.randomness) }}: jitter on the ranking
        </span>
      </label>

      <label class="knob">
        <span class="knobLabel">
          Mode
          <em v-if="knobs.mode == null">default</em>
        </span>
        <select
          :value="knobs.mode || 'similar'"
          @change="commit({ mode: $event.target.value })"
        >
          <option value="similar">Similar</option>
          <option value="explore" :disabled="hasDestination">
            Explore{{ hasDestination ? " (needs no destination)" : "" }}
          </option>
        </select>
        <span v-if="exploreConflict" class="knobHint warning" role="alert">
          Explore avoids the closest matches, so it never reaches a destination.
          Switch to Similar or clear the destination.
        </span>
        <span v-else class="knobHint">
          Explore picks related but less obvious tracks
        </span>
      </label>
    </div>

    <div class="criteriaSection">
      <div class="cardHeader">
        <span class="knobLabel">
          Criteria
          <em v-if="!knobs.criteria">default</em>
        </span>
      </div>
      <p v-if="optionsError" class="cardHint">{{ optionsError }}</p>
      <p v-else-if="!options" class="cardHint">Loading criteria…</p>
      <div
        v-for="criterion in criteriaRows"
        :key="criterion.namespace"
        class="criterionRow"
      >
        <span>{{ criterion.label }}</span>
        <input
          type="range"
          min="0"
          max="1"
          step="0.05"
          :value="criterion.weight"
          :aria-label="`${criterion.label} weight`"
          @input="setDraftCriterion(criterion.namespace, $event.target.value)"
          @change="commitCriteria"
        />
        <strong>{{ format(criterion.weight) }}</strong>
      </div>
    </div>

    <div class="awaySection">
      <ReferenceEditor
        title="Away from"
        :modelValue="knobs.away || []"
        :allowManualAdd="false"
        emptyText="Nothing to avoid."
        @update:modelValue="commit({ away: $event })"
      />
      <ReferencePicker
        placeholder="Search an artist, album or track to steer away from"
        @select="addAway"
      />
    </div>
  </section>
</template>

<script setup>
import { computed, onMounted, reactive, ref, watch } from "vue";
import ReferenceEditor from "@/components/common/ReferenceEditor.vue";
import ReferencePicker from "./ReferencePicker.vue";
import { usePlaybackStore } from "@/store/playback";
import { useRemoteStore } from "@/store/remote";

// Server-side defaults for continuation (see docs/radio-playback.md).
const DEFAULTS = { recency_weight: 0.2, diversity: 0.3, randomness: 0.3 };
const DEFAULT_NAMESPACE = "musicfm.mean.v1";
const MAX_AWAY = 8;

const props = defineProps({
  gravity: {
    type: Object,
    required: true,
  },
});

const playback = usePlaybackStore();
const remoteStore = useRemoteStore();

const options = ref(null);
const optionsError = ref("");
const knobs = computed(() => props.gravity.knobs || {});
const hasDestination = computed(() => Boolean(props.gravity.destination));
const exploreConflict = computed(
  () => hasDestination.value && knobs.value.mode === "explore",
);
const hasCustomKnobs = computed(() =>
  Object.entries(knobs.value).some(([key, value]) =>
    key === "away" ? value?.length : value != null,
  ),
);

// Slider drafts so dragging does not write (and sync) the queue on every tick.
const draft = reactive({});
const criteriaDraft = ref(null);

const resetDrafts = () => {
  for (const key of Object.keys(DEFAULTS)) delete draft[key];
  criteriaDraft.value = null;
};
watch(knobs, resetDrafts);

const displayed = computed(() => {
  const value = {};
  for (const [key, fallback] of Object.entries(DEFAULTS)) {
    value[key] =
      draft[key] ?? knobs.value[key] ?? optionDefault(key) ?? fallback;
  }
  return value;
});

const optionDefault = (key) => options.value?.[key]?.default;

const defaultNamespace = computed(() => {
  const namespaces = (options.value?.criteria || []).map((c) => c.namespace);
  return namespaces.includes(DEFAULT_NAMESPACE)
    ? DEFAULT_NAMESPACE
    : namespaces[0];
});

const criteriaRows = computed(() => {
  const source = criteriaDraft.value || knobs.value.criteria;
  const weights = new Map((source || []).map((c) => [c.namespace, c.weight]));
  return (options.value?.criteria || []).map((criterion) => ({
    namespace: criterion.namespace,
    label: criterion.label || criterion.namespace,
    weight: source
      ? (weights.get(criterion.namespace) ?? 0)
      : criterion.namespace === defaultNamespace.value
        ? 1
        : 0,
  }));
});

const setDraftCriterion = (namespace, value) => {
  criteriaDraft.value = criteriaRows.value.map((row) => ({
    namespace: row.namespace,
    weight: row.namespace === namespace ? Number(value) : row.weight,
  }));
};

const commitCriteria = () => {
  const positive = (criteriaDraft.value || []).filter((c) => c.weight > 0);
  // The server rejects non-positive weights; no positive weight means "default".
  commit({ criteria: positive.length ? positive : null });
};

const commit = (partial) => {
  if (partial.mode === "similar" && knobs.value.mode == null) {
    partial = { ...partial, mode: null };
  }
  playback.setGravityKnobs(partial);
};

const addAway = (reference) => {
  const current = knobs.value.away || [];
  if (
    current.some(
      (item) =>
        item.entity_type === reference.entity_type &&
        item.entity_id === reference.entity_id,
    )
  )
    return;
  commit({ away: [...current, { ...reference, weight: 1 }].slice(-MAX_AWAY) });
};

const resetKnobs = () =>
  playback.setGravityKnobs({
    recency_weight: null,
    criteria: null,
    diversity: null,
    randomness: null,
    mode: null,
    away: [],
  });

const format = (value) => Number(value ?? 0).toFixed(2);

onMounted(async () => {
  options.value = await remoteStore.fetchRadioOptions();
  if (!options.value) optionsError.value = "Could not load criteria.";
});
</script>

<style scoped>
@import "./steeringCard.css";

.knobGrid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
  gap: 16px;
}

.knob {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.knobLabel {
  color: var(--text-bright);
  font-weight: 700;
}

.knobLabel em {
  margin-left: 6px;
  color: var(--text-subdued);
  font-size: 0.75rem;
  font-style: normal;
  font-weight: 400;
}

.knobHint {
  color: var(--text-subdued);
  font-size: 0.8rem;
  line-height: 1.35;
}

.knobHint.warning {
  color: var(--warning-color, #f0b35b);
}

select {
  min-height: 34px;
  border: 1px solid var(--border-default);
  border-radius: var(--radius-md);
  background: var(--bg-highlight);
  color: var(--text-bright);
  padding: 6px 10px;
}

.criteriaSection,
.awaySection {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.criterionRow {
  display: grid;
  grid-template-columns: minmax(110px, 160px) 1fr 44px;
  align-items: center;
  gap: 10px;
}

.criterionRow span {
  color: var(--text-subdued);
}

.criterionRow strong {
  text-align: right;
}
</style>
