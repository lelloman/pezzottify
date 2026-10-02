<template>
  <section class="steeringCard">
    <header class="cardHeader">
      <span class="cardTitle">Source</span>
      <button
        v-if="gravity.source.kind === 'references'"
        type="button"
        class="textButton"
        @click="resetToQueue"
      >
        Reset to queue
      </button>
    </header>

    <template v-if="gravity.source.kind === 'queue'">
      <p class="sourceSummary">
        Your queue:
        <strong>{{ userChosenCount }}</strong>
        {{ userChosenCount === 1 ? "track" : "tracks" }} you chose
        <template v-if="suggestedCount">
          , <strong>{{ suggestedCount }}</strong> added by smart continuation
        </template>
      </p>
      <p class="cardHint">
        Suggestions follow the tracks you chose, with a small pull from what
        just played. Tracks added by smart continuation never become the source.
      </p>
    </template>

    <template v-else>
      <ReferenceEditor
        title="Anchored on"
        :modelValue="gravity.source.references"
        :allowManualAdd="false"
        emptyText="No references: suggestions fall back to the queue."
        @update:modelValue="updateReferences"
      />
    </template>

    <details class="anchorPicker">
      <summary>Anchor on something else</summary>
      <ReferencePicker
        placeholder="Search an artist, album or track to anchor on"
        @select="addReference"
      />
    </details>
  </section>
</template>

<script setup>
import { computed } from "vue";
import ReferenceEditor from "@/components/common/ReferenceEditor.vue";
import ReferencePicker from "./ReferencePicker.vue";
import { usePlaybackStore } from "@/store/playback";

const props = defineProps({
  gravity: {
    type: Object,
    required: true,
  },
  tracksIds: {
    type: Array,
    default: () => [],
  },
});

const playback = usePlaybackStore();

const autoIds = computed(() => new Set(props.gravity.auto_track_ids || []));
const suggestedCount = computed(
  () => props.tracksIds.filter((id) => autoIds.value.has(id)).length,
);
const userChosenCount = computed(
  () => new Set(props.tracksIds.filter((id) => !autoIds.value.has(id))).size,
);

const updateReferences = (references) => {
  playback.setGravitySource(
    references.length ? { kind: "references", references } : { kind: "queue" },
  );
};

const addReference = (reference) => {
  const current =
    props.gravity.source.kind === "references"
      ? props.gravity.source.references
      : [];
  if (
    current.some(
      (item) =>
        item.entity_type === reference.entity_type &&
        item.entity_id === reference.entity_id,
    )
  )
    return;
  updateReferences([...current, { ...reference, weight: 1 }].slice(-8));
};

const resetToQueue = () => playback.setGravitySource({ kind: "queue" });
</script>

<style scoped>
@import "./steeringCard.css";

.sourceSummary {
  margin: 0;
  color: var(--text-bright);
}

.anchorPicker summary {
  cursor: pointer;
  color: var(--text-subdued);
  font-size: 0.85rem;
}

.anchorPicker[open] summary {
  margin-bottom: 8px;
}
</style>
