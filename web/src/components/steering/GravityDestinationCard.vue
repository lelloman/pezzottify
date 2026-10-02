<template>
  <section class="steeringCard">
    <header class="cardHeader">
      <span class="cardTitle">Destination</span>
      <button
        v-if="destination"
        type="button"
        class="textButton"
        @click="playback.clearGravityDestination()"
      >
        Clear
      </button>
    </header>

    <template v-if="destination">
      <div class="destinationIdentity">
        <span class="destinationType">{{ destination.entity_type }}</span>
        <RouterLink class="destinationLabel" :to="destinationRoute">
          {{ destination.label || destination.entity_id }}
        </RouterLink>
      </div>

      <div
        class="progressTrack"
        role="progressbar"
        aria-label="Steering progress"
        :aria-valuenow="progressPercent"
        aria-valuemin="0"
        aria-valuemax="100"
      >
        <div class="progressFill" :style="{ width: progressPercent + '%' }" />
      </div>
      <div class="progressNumbers">
        <span>{{ progressPercent }}% of the way</span>
        <span>
          {{ gravity.steps_done }} / {{ gravity.steps_total }} tracks
        </span>
      </div>

      <dl v-if="diagnostics" class="diagnostics">
        <div>
          <dt>Last pick aimed at</dt>
          <dd>
            {{ percent(diagnostics.query_to_destination) }} destination,
            {{ percent(diagnostics.query_to_source) }} source
          </dd>
        </div>
        <div v-if="diagnostics.source_to_destination != null">
          <dt>Source to destination similarity</dt>
          <dd>{{ percent(diagnostics.source_to_destination) }}</dd>
        </div>
      </dl>
      <p v-else class="cardHint">
        Similarities appear after smart continuation adds the next tracks.
      </p>

      <label class="fieldRow">
        <span>Tracks remaining</span>
        <input
          class="numberInput"
          type="number"
          min="0"
          :value="remaining"
          @change="updateRemaining($event.target.value)"
        />
      </label>
      <p class="cardHint">
        When it arrives, the destination becomes the new source.
      </p>
    </template>

    <template v-else>
      <p class="cardHint">
        Pick where the queue should drift to. Each track smart continuation adds
        moves the suggestions a step closer.
      </p>
      <label class="fieldRow">
        <span>Steps</span>
        <input
          v-model.number="newSteps"
          class="numberInput"
          type="number"
          min="1"
          max="500"
        />
      </label>
    </template>

    <details v-if="destination" class="destinationPicker">
      <summary>Change destination</summary>
      <ReferencePicker
        placeholder="Search a destination artist, album or track"
        @select="setDestination"
      />
    </details>
    <ReferencePicker
      v-else
      placeholder="Search a destination artist, album or track"
      @select="setDestination"
    />
  </section>
</template>

<script setup>
import { computed, ref, watch } from "vue";
import ReferencePicker from "./ReferencePicker.vue";
import { usePlaybackStore } from "@/store/playback";
import { progress } from "@/utils/gravity";

const props = defineProps({
  gravity: {
    type: Object,
    required: true,
  },
});

const playback = usePlaybackStore();

const destination = computed(() => props.gravity.destination);
const destinationRoute = computed(
  () => `/${destination.value.entity_type}/${destination.value.entity_id}`,
);
const progressPercent = computed(() =>
  Math.round(progress(props.gravity) * 100),
);
const remaining = computed(() =>
  Math.max(0, props.gravity.steps_total - props.gravity.steps_done),
);
const diagnostics = computed(
  () => props.gravity.last_diagnostics?.namespaces?.[0] ?? null,
);

const newSteps = ref(props.gravity.steps_total);
watch(
  () => props.gravity.steps_total,
  (total) => {
    newSteps.value = total;
  },
);

const percent = (value) =>
  value == null ? "n/a" : `${Math.round(Math.max(0, value) * 100)}%`;

const updateRemaining = (value) => {
  const parsed = Math.floor(Number(value));
  if (!Number.isFinite(parsed) || parsed < 0) return;
  // Zero remaining arrives immediately: the destination becomes the source.
  playback.setGravityStepsTotal(props.gravity.steps_done + parsed);
};

const setDestination = (reference) => {
  const steps = destination.value
    ? Math.max(1, remaining.value)
    : Math.max(1, Math.floor(Number(newSteps.value) || 1));
  playback.setGravityDestination(reference, steps);
};
</script>

<style scoped>
@import "./steeringCard.css";

.destinationIdentity {
  display: flex;
  align-items: baseline;
  gap: 10px;
  min-width: 0;
}

.destinationType {
  color: var(--text-subdued);
  font-size: 0.75rem;
  text-transform: uppercase;
}

.destinationLabel {
  min-width: 0;
  overflow: hidden;
  color: var(--text-bright);
  font-size: 1.25rem;
  font-weight: 800;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.progressTrack {
  height: 8px;
  overflow: hidden;
  border-radius: 999px;
  background: var(--bg-highlight);
}

.progressFill {
  height: 100%;
  border-radius: inherit;
  background: var(--spotify-green);
  transition: width var(--transition-fast, 0.15s);
}

.progressNumbers {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  color: var(--text-subdued);
  font-size: 0.85rem;
}

.diagnostics {
  display: grid;
  gap: 6px;
  margin: 0;
}

.diagnostics div {
  display: flex;
  flex-wrap: wrap;
  justify-content: space-between;
  gap: 8px;
}

.diagnostics dt {
  color: var(--text-subdued);
}

.diagnostics dd {
  margin: 0;
  color: var(--text-bright);
}

.destinationPicker summary {
  cursor: pointer;
  color: var(--text-subdued);
  font-size: 0.85rem;
}

.destinationPicker[open] summary {
  margin-bottom: 8px;
}
</style>
