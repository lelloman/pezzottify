<template>
  <section class="steeringSection">
    <header class="sectionHeader">
      <h2 class="sectionTitle">Starting point</h2>
      <button
        v-if="gravity.source.kind === 'references'"
        type="button"
        class="textLink"
        @click="resetToQueue"
      >
        Use my queue again
      </button>
    </header>
    <p class="sectionHint">
      What the queue sounds like now: the tracks you chose, or where a previous
      journey arrived.
    </p>

    <div v-if="gravity.source.kind === 'queue'" class="queueRow">
      <div class="queueArt">
        <QueueCollage :tracksIds="chosenTracksIds" />
      </div>
      <div class="queueText">
        <span class="queueTitle">Your queue</span>
        <span class="queueMeta">
          {{ userChosenCount }}
          {{ userChosenCount === 1 ? "track" : "tracks" }} you chose<template
            v-if="suggestedCount"
          >
            · {{ suggestedCount }} added by smart continuation</template
          >
        </span>
      </div>
    </div>
    <p v-if="gravity.source.kind === 'queue'" class="mutedHint">
      Tracks added by smart continuation never change the starting point.
    </p>

    <template v-else>
      <span class="basedOn">Based on</span>
      <ul class="chipList">
        <SteeringChip
          v-for="reference in references"
          :key="reference.entity_type + ':' + reference.entity_id"
          :reference="reference"
          @remove="removeReference(reference)"
        />
      </ul>
    </template>

    <div class="actionRow">
      <button
        type="button"
        class="pill"
        :aria-expanded="pickerOpen"
        @click="pickerOpen = !pickerOpen"
      >
        {{ pickerOpen ? "Done" : "Start from something else" }}
      </button>
    </div>
    <ReferencePicker
      v-if="pickerOpen"
      placeholder="Search something to start from"
      @select="addReference"
    />
  </section>
</template>

<script setup>
import { computed, ref } from "vue";
import QueueCollage from "./QueueCollage.vue";
import ReferencePicker from "./ReferencePicker.vue";
import SteeringChip from "./SteeringChip.vue";
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

const pickerOpen = ref(false);
const references = computed(() =>
  props.gravity.source.kind === "references"
    ? props.gravity.source.references || []
    : [],
);
const chosenTracksIds = computed(() =>
  props.tracksIds.filter((id) => !autoIds.value.has(id)),
);
const removeReference = (reference) =>
  updateReferences(
    references.value.filter(
      (item) =>
        item.entity_type !== reference.entity_type ||
        item.entity_id !== reference.entity_id,
    ),
  );
</script>

<style scoped>
@import "./steeringCard.css";

.queueRow {
  display: flex;
  align-items: center;
  gap: 16px;
  min-width: 0;
}

.queueArt {
  flex: 0 0 72px;
  width: 72px;
}

.queueArt :deep(.queueCollage) {
  border-radius: var(--radius-md);
  box-shadow: var(--shadow-md);
}

.queueText {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
}

.queueTitle {
  color: var(--text-bright);
  font-size: var(--text-lg);
  font-weight: var(--font-bold);
}

.queueMeta {
  color: var(--text-subdued);
  font-size: var(--text-sm);
}

.basedOn {
  color: var(--text-subdued);
  font-size: var(--text-xs);
  font-weight: var(--font-bold);
  letter-spacing: 0.08em;
  text-transform: uppercase;
}
</style>
