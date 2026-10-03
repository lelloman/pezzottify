<template>
  <section class="steeringCard">
    <header class="cardHeader">
      <span class="cardTitle">Heading to</span>
      <button
        v-if="components.length"
        type="button"
        class="textButton"
        @click="playback.clearGravityDestination()"
      >
        Stop steering
      </button>
    </header>

    <template v-if="components.length">
      <ul class="mixList">
        <li
          v-for="component in components"
          :key="componentKey(component)"
          class="mixComponent"
        >
          <div class="componentHeader">
            <span class="componentBadge">{{ componentBadge(component) }}</span>
            <RouterLink
              v-if="component.entity_type !== 'concept'"
              class="componentLabel"
              :to="`/${component.entity_type}/${component.entity_id}`"
            >
              {{ component.label || component.entity_id }}
            </RouterLink>
            <span v-else class="componentLabel">
              {{ component.label || component.entity_id }}
            </span>
            <span
              class="componentCloseness"
              :title="'How close the last pick was to this'"
            >
              {{ closenessFor(component) }}
            </span>
            <button
              type="button"
              class="removeButton"
              :aria-label="`Remove ${component.label || component.entity_id}`"
              @click="playback.removeGravityDestinationComponent(component)"
            >
              ×
            </button>
          </div>
          <label v-if="components.length > 1" class="weightRow">
            <span>Share</span>
            <input
              type="range"
              min="0.1"
              max="2"
              step="0.1"
              :value="component.weight ?? 1"
              :aria-label="`Weight of ${component.label || component.entity_id}`"
              @change="
                playback.setGravityDestinationComponentWeight(
                  component,
                  Number($event.target.value),
                )
              "
            />
            <span class="weightValue">{{ shareOf(component) }}%</span>
          </label>
        </li>
      </ul>

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
          {{ gravity.steps_done }} of {{ gravity.steps_total }} tracks along the
          way
        </span>
      </div>

      <dl v-if="diagnostics" class="diagnostics">
        <div>
          <dt>Last pick</dt>
          <dd>
            {{ percent(diagnostics.query_to_destination) }} like where it's
            heading, {{ percent(diagnostics.query_to_source) }} like the
            starting point
          </dd>
        </div>
        <div v-if="diagnostics.source_to_destination != null">
          <dt>Starting point and destination</dt>
          <dd>{{ percent(diagnostics.source_to_destination) }} alike</dd>
        </div>
      </dl>
      <p v-else class="cardHint">
        Progress details appear after smart continuation adds the next tracks.
      </p>

      <label class="fieldRow">
        <span>Get there in</span>
        <input
          class="numberInput"
          type="number"
          min="0"
          :value="remaining"
          @change="updateRemaining($event.target.value)"
        />
        <span>more tracks</span>
      </label>
      <p class="cardHint">
        When it gets there, this becomes the new starting point.
      </p>

      <details
        v-if="components.length < MAX_COMPONENTS"
        class="destinationPicker"
      >
        <summary>Add to the mix</summary>
        <ReferencePicker
          placeholder="Search something to add"
          @select="addComponent"
        />
      </details>
      <p v-else class="cardHint">
        A mix holds at most {{ MAX_COMPONENTS }} parts.
      </p>
    </template>

    <template v-else>
      <p class="cardHint">
        Not steering. Pick an artist, album, track or concept to head toward.
        You can mix several.
      </p>
      <label class="fieldRow">
        <span>Get there in</span>
        <input
          v-model.number="newSteps"
          class="numberInput"
          type="number"
          min="1"
          max="500"
        />
        <span>tracks</span>
      </label>
      <ReferencePicker
        placeholder="Search where to head"
        @select="startDestination"
      />
    </template>
  </section>
</template>

<script setup>
import { computed, ref, watch } from "vue";
import ReferencePicker from "./ReferencePicker.vue";
import { usePlaybackStore } from "@/store/playback";
import { MAX_COMPONENTS, progress } from "@/utils/gravity";
import { componentBadge } from "@/utils/concepts";

const props = defineProps({
  gravity: {
    type: Object,
    required: true,
  },
});

const playback = usePlaybackStore();

const components = computed(() => props.gravity.destination || []);
const progressPercent = computed(() =>
  Math.round(progress(props.gravity) * 100),
);
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

const percent = (value) =>
  value == null ? "n/a" : `${Math.round(Math.max(0, value) * 100)}%`;

const updateRemaining = (value) => {
  const parsed = Math.floor(Number(value));
  if (!Number.isFinite(parsed) || parsed < 0) return;
  // Zero remaining arrives immediately: the mix becomes the source.
  playback.setGravityStepsTotal(props.gravity.steps_done + parsed);
};

const startDestination = (reference) => {
  const steps = Math.max(1, Math.floor(Number(newSteps.value) || 1));
  playback.setGravityDestination(reference, steps);
};

const addComponent = (reference) => {
  playback.addGravityDestinationComponent(reference);
};
</script>

<style scoped>
@import "./steeringCard.css";

.mixList {
  display: flex;
  flex-direction: column;
  gap: 10px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.mixComponent {
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 8px 10px;
  border-radius: var(--radius-md);
  background: var(--bg-highlight);
}

.componentHeader {
  display: flex;
  align-items: baseline;
  gap: 10px;
  min-width: 0;
}

.componentBadge {
  flex: none;
  color: var(--text-subdued);
  font-size: 0.72rem;
  text-transform: uppercase;
}

.componentLabel {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  color: var(--text-bright);
  font-size: 1.05rem;
  font-weight: 800;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.componentCloseness {
  flex: none;
  color: #9eddb7;
  font-size: 0.82rem;
}

.removeButton {
  flex: none;
  width: 26px;
  height: 26px;
  border-radius: 50%;
  background: transparent;
  color: var(--text-subdued);
  font-size: 1.1rem;
  line-height: 1;
}

.removeButton:hover {
  background: var(--bg-press);
  color: var(--text-bright);
}

.weightRow {
  display: flex;
  align-items: center;
  gap: 10px;
  color: var(--text-subdued);
  font-size: 0.82rem;
}

.weightRow input {
  flex: 1;
  min-width: 0;
}

.weightValue {
  width: 40px;
  text-align: right;
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
