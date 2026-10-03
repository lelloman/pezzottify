<template>
  <section class="steeringSection">
    <header class="sectionHeader">
      <h2 class="sectionTitle">Along the way</h2>
      <button
        type="button"
        class="textLink"
        :disabled="!customised"
        @click="reset"
      >
        Reset to defaults
      </button>
    </header>

    <div class="journeyGrid">
      <div class="control">
        <span class="controlLabel">Avoid</span>
        <span class="mutedHint">Steer away from these.</span>
        <ul v-if="(knobs.away || []).length" class="chipList">
          <SteeringChip
            v-for="reference in knobs.away"
            :key="reference.entity_type + ':' + reference.entity_id"
            :reference="reference"
            @remove="removeAway(reference)"
          />
        </ul>
        <div class="actionRow">
          <button
            type="button"
            class="pill"
            :aria-expanded="avoidPickerOpen"
            @click="avoidPickerOpen = !avoidPickerOpen"
          >
            {{ avoidPickerOpen ? "Done" : "Add something to avoid" }}
          </button>
        </div>
        <ReferencePicker
          v-if="avoidPickerOpen"
          placeholder="Search something to avoid"
          @select="addAway"
        />
      </div>

      <div class="sliders">
        <label class="control">
          <span class="controlLabel">Follow what's playing</span>
          <input
            class="steerRange"
            type="range"
            min="0"
            max="1"
            step="0.05"
            :value="follow"
            :style="{ '--fill': fill(follow) }"
            @input="draft.follow = Number($event.target.value)"
            @change="write(followPartial(draft.follow))"
          />
          <span class="sliderEnds">
            <span>Stay on the starting point</span>
            <span>Follow recent tracks</span>
          </span>
          <span class="mutedHint">
            How much the last few tracks pull on the next pick.
          </span>
        </label>

        <label class="control">
          <span class="controlLabel">Variety</span>
          <input
            class="steerRange"
            type="range"
            min="0"
            max="1"
            step="0.05"
            :value="variety"
            :style="{ '--fill': fill(variety) }"
            @input="draft.variety = Number($event.target.value)"
            @change="write(varietyPartial(draft.variety))"
          />
          <span class="sliderEnds">
            <span>Focused</span>
            <span>Varied</span>
          </span>
          <span class="mutedHint">
            Higher mixes in other artists and less obvious picks.
          </span>
        </label>
      </div>
    </div>

    <details class="quietDisclosure moreOptions">
      <summary>More options</summary>
      <div class="listenFor">
        <span class="controlLabel">What to listen for</span>
        <span class="mutedHint">
          Which qualities of the music count when picking what's similar.
        </span>
        <p v-if="optionsError" class="mutedHint">{{ optionsError }}</p>
        <p v-else-if="!options" class="mutedHint">Loading…</p>
        <div
          v-for="row in listenRows"
          :key="row.namespace"
          class="listenRow"
        >
          <span class="listenLabel">{{ row.label }}</span>
          <input
            class="steerRange"
            type="range"
            min="0"
            max="1"
            step="0.05"
            :value="row.weight"
            :style="{ '--fill': fill(row.weight) }"
            :aria-label="`${row.label} importance`"
            @input="setDraftListen(row.namespace, $event.target.value)"
            @change="commitListen"
          />
          <span class="listenShare">{{ shareOf(row) }}%</span>
        </div>
      </div>
    </details>
  </section>
</template>

<script setup>
import { computed, onMounted, reactive, ref, watch } from "vue";
import ReferencePicker from "./ReferencePicker.vue";
import SteeringChip from "./SteeringChip.vue";
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

const avoidPickerOpen = ref(false);
const removeAway = (reference) =>
  write({
    away: (knobs.value.away || []).filter(
      (item) =>
        item.entity_type !== reference.entity_type ||
        item.entity_id !== reference.entity_id,
    ),
  });

const fill = (value) => `${Math.round(Number(value) * 100)}%`;

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

.journeyGrid {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  gap: 32px;
}

.sliders {
  display: flex;
  flex-direction: column;
  gap: 24px;
}

.control {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-width: 0;
}

.controlLabel {
  color: var(--text-bright);
  font-size: var(--text-md);
  font-weight: var(--font-bold);
}

.sliderEnds {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  color: var(--text-subdued);
  font-size: var(--text-xs);
  font-weight: var(--font-semibold);
}

.sliderEnds span:last-child {
  text-align: right;
}

.moreOptions[open] .listenFor {
  margin-top: 16px;
}

.listenFor {
  display: flex;
  flex-direction: column;
  gap: 10px;
  max-width: 560px;
}

.listenRow {
  display: grid;
  grid-template-columns: 120px minmax(0, 1fr) 44px;
  align-items: center;
  gap: 14px;
}

.listenLabel {
  color: var(--text-bright);
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
}

.listenShare {
  color: var(--text-subdued);
  font-size: var(--text-xs);
  font-variant-numeric: tabular-nums;
  text-align: right;
}

@media (max-width: 900px) {
  .journeyGrid {
    grid-template-columns: minmax(0, 1fr);
  }
}

@media (max-width: 600px) {
  .listenRow {
    grid-template-columns: 96px minmax(0, 1fr) 40px;
    gap: 10px;
  }
}
</style>
