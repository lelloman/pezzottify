<template>
  <section class="steeringCard">
    <header class="cardHeader">
      <span class="cardTitle">Along the way</span>
      <button
        type="button"
        class="textButton"
        :disabled="!customised"
        @click="reset"
      >
        Reset to defaults
      </button>
    </header>

    <div class="journeySection">
      <ReferenceEditor
        title="Avoid"
        :modelValue="knobs.away || []"
        :allowManualAdd="false"
        emptyText="Nothing to avoid."
        @update:modelValue="write({ away: $event })"
      />
      <p class="cardHint">Steer away from these.</p>
      <ReferencePicker
        placeholder="Search something to avoid"
        @select="addAway"
      />
    </div>

    <div class="sliderGrid">
      <label class="sliderControl">
        <span class="controlLabel">Follow what's playing</span>
        <input
          type="range"
          min="0"
          max="1"
          step="0.05"
          :value="follow"
          @input="draft.follow = Number($event.target.value)"
          @change="write(followPartial(draft.follow))"
        />
        <span class="sliderEnds">
          <span>Stay on the starting point</span>
          <span>Follow recent tracks</span>
        </span>
        <span class="cardHint">
          How much the last few tracks pull on the next pick.
        </span>
      </label>

      <label class="sliderControl">
        <span class="controlLabel">Variety</span>
        <input
          type="range"
          min="0"
          max="1"
          step="0.05"
          :value="variety"
          @input="draft.variety = Number($event.target.value)"
          @change="write(varietyPartial(draft.variety))"
        />
        <span class="sliderEnds">
          <span>Focused</span>
          <span>Varied</span>
        </span>
        <span class="cardHint">
          Higher mixes in other artists and less obvious picks.
        </span>
      </label>
    </div>

    <details class="moreOptions">
      <summary>More options</summary>
      <div class="listenFor">
        <span class="controlLabel">What to listen for</span>
        <p class="cardHint">
          Which qualities of the music count when picking what's similar.
        </p>
        <p v-if="optionsError" class="cardHint">{{ optionsError }}</p>
        <p v-else-if="!options" class="cardHint">Loading…</p>
        <div
          v-for="row in listenRows"
          :key="row.namespace"
          class="listenRow"
        >
          <span>{{ row.label }}</span>
          <input
            type="range"
            min="0"
            max="1"
            step="0.05"
            :value="row.weight"
            :aria-label="`${row.label} importance`"
            @input="setDraftListen(row.namespace, $event.target.value)"
            @change="commitListen"
          />
          <strong>{{ shareOf(row) }}%</strong>
        </div>
      </div>
    </details>
  </section>
</template>

<script setup>
import { computed, onMounted, reactive, ref, watch } from "vue";
import ReferenceEditor from "@/components/common/ReferenceEditor.vue";
import ReferencePicker from "./ReferencePicker.vue";
import { usePlaybackStore } from "@/store/playback";
import { useRemoteStore } from "@/store/remote";
import {
  JOURNEY_DEFAULTS,
  RESET_JOURNEY,
  displayedFollow,
  displayedVariety,
  followPartial,
  hasCustomJourney,
  journeyPartial,
  listenForLabel,
  varietyPartial,
} from "@/utils/journey";

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
const customised = computed(() => hasCustomJourney(knobs.value));

// Slider drafts so dragging does not write (and sync) the queue on every tick.
const draft = reactive({ follow: null, variety: null });
const listenDraft = ref(null);
watch(knobs, () => {
  draft.follow = null;
  draft.variety = null;
  listenDraft.value = null;
});

const follow = computed(
  () =>
    draft.follow ??
    displayedFollow(knobs.value, JOURNEY_DEFAULTS.follow),
);
const variety = computed(
  () =>
    draft.variety ??
    displayedVariety(
      knobs.value,
      options.value?.diversity?.default ?? JOURNEY_DEFAULTS.variety,
    ),
);

const write = (partial) => playback.setGravityKnobs(journeyPartial(partial));

const reset = () => playback.setGravityKnobs({ ...RESET_JOURNEY, away: [] });

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
  write({ away: [...current, { ...reference, weight: 1 }].slice(-MAX_AWAY) });
};

const defaultNamespace = computed(() => {
  const namespaces = (options.value?.criteria || []).map((c) => c.namespace);
  return namespaces.includes(DEFAULT_NAMESPACE)
    ? DEFAULT_NAMESPACE
    : namespaces[0];
});

const listenRows = computed(() => {
  const source = listenDraft.value || knobs.value.criteria;
  const weights = new Map((source || []).map((c) => [c.namespace, c.weight]));
  return (options.value?.criteria || []).map((criterion) => ({
    namespace: criterion.namespace,
    label: listenForLabel(criterion.namespace, criterion.label),
    weight: source
      ? (weights.get(criterion.namespace) ?? 0)
      : criterion.namespace === defaultNamespace.value
        ? 1
        : 0,
  }));
});

const totalListen = computed(() =>
  listenRows.value.reduce((sum, row) => sum + row.weight, 0),
);
const shareOf = (row) =>
  Math.round((row.weight / (totalListen.value || 1)) * 100);

const setDraftListen = (namespace, value) => {
  listenDraft.value = listenRows.value.map((row) => ({
    namespace: row.namespace,
    weight: row.namespace === namespace ? Number(value) : row.weight,
  }));
};

const commitListen = () => {
  const positive = (listenDraft.value || []).filter((c) => c.weight > 0);
  // The server rejects non-positive weights; nothing selected means "default".
  write({ criteria: positive.length ? positive : null });
};

onMounted(async () => {
  options.value = await remoteStore.fetchRadioOptions();
  if (!options.value) optionsError.value = "Could not load these options.";
});
</script>

<style scoped>
@import "./steeringCard.css";

.journeySection,
.listenFor {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.sliderGrid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
  gap: 18px;
}

.sliderControl {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.controlLabel {
  color: var(--text-bright);
  font-weight: 700;
}

.sliderEnds {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  color: var(--text-subdued);
  font-size: 0.75rem;
}

.sliderEnds span:last-child {
  text-align: right;
}

.moreOptions summary {
  cursor: pointer;
  color: var(--text-subdued);
  font-weight: 600;
}

.moreOptions[open] summary {
  margin-bottom: 10px;
}

.listenRow {
  display: grid;
  grid-template-columns: minmax(110px, 160px) 1fr 44px;
  align-items: center;
  gap: 10px;
}

.listenRow span {
  color: var(--text-subdued);
}

.listenRow strong {
  text-align: right;
}
</style>
