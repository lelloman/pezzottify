<template>
  <section class="steeringSection">
    <header class="sectionHeader">
      <h2 class="sectionTitle">Heading to</h2>
      <button
        v-if="components.length"
        type="button"
        class="textLink"
        @click="playback.clearGravityDestination()"
      >
        Stop steering
      </button>
    </header>
    <p class="sectionHint">
      <template v-if="components.length">
        Smart continuation drifts toward this mix.
        <template v-if="components.length > 1">
          Tap a part to change its share.
        </template>
      </template>
      <template v-else>
        Not steering. Pick an artist, album, track or concept to head toward.
        You can mix several.
      </template>
    </p>

    <ul v-if="components.length" class="chipList">
      <SteeringChip
        v-for="component in components"
        :key="componentKey(component)"
        :reference="component"
        :share="components.length > 1 ? shareOf(component) : null"
        :closeness="closenessFor(component)"
        :editableWeight="components.length > 1"
        :weight="component.weight ?? 1"
        @remove="playback.removeGravityDestinationComponent(component)"
        @weight="playback.setGravityDestinationComponentWeight(component, $event)"
      />
    </ul>

    <p v-if="components.length && diagnostics" class="lastPick">
      Last pick:
      <strong>{{ percent(diagnostics.query_to_destination) }}</strong> like
      where it's heading,
      <strong>{{ percent(diagnostics.query_to_source) }}</strong> like the
      starting point.
      <template v-if="diagnostics.source_to_destination != null">
        Starting point and destination are
        {{ percent(diagnostics.source_to_destination) }} alike.
      </template>
    </p>
    <p v-else-if="components.length" class="mutedHint">
      Progress details appear after smart continuation adds the next tracks.
    </p>

    <div class="stepper">
      <span>Get there in</span>
      <span class="stepperControl">
        <button
          type="button"
          aria-label="Fewer tracks"
          :disabled="stepsValue <= stepsMin"
          @click="setSteps(stepsValue - 1)"
        >
          −
        </button>
        <input
          type="number"
          :min="stepsMin"
          max="500"
          :value="stepsValue"
          aria-label="Tracks to get there"
          @change="setSteps($event.target.value)"
        />
        <button
          type="button"
          aria-label="More tracks"
          :disabled="stepsValue >= 500"
          @click="setSteps(stepsValue + 1)"
        >
          +
        </button>
      </span>
      <span>{{ components.length ? "more tracks" : "tracks" }}</span>
    </div>
    <p v-if="components.length" class="mutedHint">
      When it gets there, this becomes the new starting point.
    </p>

    <div class="actionRow">
      <button
        v-if="components.length"
        type="button"
        class="pill"
        :disabled="components.length >= MAX_COMPONENTS"
        :aria-expanded="pickerOpen"
        @click="pickerOpen = !pickerOpen"
      >
        {{ pickerOpen ? "Done adding" : "Add to the mix" }}
      </button>
      <button
        v-else-if="!pickerOpen"
        type="button"
        class="pill primary"
        @click="pickerOpen = true"
      >
        Pick where to head
      </button>
      <span v-if="components.length >= MAX_COMPONENTS" class="mutedHint">
        A mix holds at most {{ MAX_COMPONENTS }} parts.
      </span>
    </div>

    <ReferencePicker
      v-if="pickerOpen"
      :placeholder="
        components.length ? 'Search something to add' : 'Search where to head'
      "
      @select="components.length ? addComponent($event) : startDestination($event)"
    />
  </section>
</template>

<script setup>
import { computed, ref, watch } from "vue";
import ReferencePicker from "./ReferencePicker.vue";
import SteeringChip from "./SteeringChip.vue";
import { usePlaybackStore } from "@/store/playback";
import { MAX_COMPONENTS } from "@/utils/gravity";

const props = defineProps({
  gravity: {
    type: Object,
    required: true,
  },
});

const playback = usePlaybackStore();

const pickerOpen = ref(false);
const components = computed(() => props.gravity.destination || []);
const remaining = computed(() =>
  Math.max(0, props.gravity.steps_total - props.gravity.steps_done),
);
const diagnostics = computed(
  () => props.gravity.last_diagnostics?.namespaces?.[0] ?? null,
);

const componentKey = (component) =>
  `${component.entity_type}:${component.entity_id}`;

const totalWeight = computed(() =>
  components.value.reduce((sum, component) => sum + (component.weight ?? 1), 0),
);
const shareOf = (component) =>
  Math.round(((component.weight ?? 1) / (totalWeight.value || 1)) * 100);

const percent = (value) =>
  value == null ? "n/a" : `${Math.round(Math.max(0, value) * 100)}%`;

const closenessFor = (component) => {
  const match = diagnostics.value?.destination_components?.find(
    (entry) =>
      entry.entity_type === component.entity_type &&
      entry.entity_id === component.entity_id,
  );
  if (!match || match.similarity == null) return "";
  return percent(match.similarity);
};

const newSteps = ref(props.gravity.steps_total);
watch(
  () => props.gravity.steps_total,
  (total) => {
    newSteps.value = total;
  },
);

// While steering, the stepper edits the tracks still to go (0 arrives at once);
// before steering, it sets how many tracks the new journey takes.
const stepsMin = computed(() => (components.value.length ? 0 : 1));
const stepsValue = computed(() =>
  components.value.length ? remaining.value : newSteps.value,
);

const setSteps = (value) => {
  const parsed = Math.floor(Number(value));
  if (!Number.isFinite(parsed)) return;
  const clamped = Math.min(500, Math.max(stepsMin.value, parsed));
  if (components.value.length) {
    playback.setGravityStepsTotal(props.gravity.steps_done + clamped);
  } else {
    newSteps.value = clamped;
  }
};

const startDestination = (reference) => {
  const steps = Math.max(1, Math.floor(Number(newSteps.value) || 1));
  playback.setGravityDestination(reference, steps);
  pickerOpen.value = false;
};

const addComponent = (reference) => {
  playback.addGravityDestinationComponent(reference);
  if (components.value.length + 1 >= MAX_COMPONENTS) pickerOpen.value = false;
};

defineExpose({
  openPicker: () => {
    pickerOpen.value = true;
  },
});
</script>

<style scoped>
@import "./steeringCard.css";

.lastPick {
  margin: 0;
  color: var(--text-subdued);
  font-size: var(--text-sm);
  line-height: 1.5;
}

.lastPick strong {
  color: var(--text-bright);
}
</style>
