<template>
  <li class="steeringChip" :class="{ expanded, editable: editableWeight }">
    <component
      :is="editableWeight ? 'button' : 'div'"
      :type="editableWeight ? 'button' : undefined"
      class="chipMain"
      :aria-expanded="editableWeight ? expanded : undefined"
      :title="editableWeight ? 'Adjust its share of the mix' : undefined"
      @click="editableWeight && (expanded = !expanded)"
    >
      <SteeringArtwork :reference="reference" size="xs" />
      <span class="chipText">
        <span class="chipLabel">{{ label }}</span>
        <span class="chipMeta">{{ badge }}</span>
      </span>
      <span v-if="share != null" class="shareBar" :title="`${share}% of the mix`">
        <span class="shareFill" :style="{ width: share + '%' }" />
      </span>
      <span v-if="closeness" class="closeness" :title="closenessTitle">
        {{ closeness }}
      </span>
    </component>
    <button
      v-if="removable"
      type="button"
      class="removeButton"
      :aria-label="`Remove ${label}`"
      @click="$emit('remove')"
    >
      ×
    </button>
    <div v-if="editableWeight && expanded" class="weightPanel">
      <span class="weightLabel">Share</span>
      <input
        class="steerRange"
        type="range"
        min="0.1"
        max="2"
        step="0.1"
        :value="draftWeight ?? weight"
        :style="{ '--fill': fillPercent + '%' }"
        :aria-label="`Share of ${label}`"
        @input="draftWeight = Number($event.target.value)"
        @change="commit"
      />
      <span class="weightValue">{{ share }}%</span>
    </div>
  </li>
</template>

<script setup>
import { computed, ref, watch } from "vue";
import SteeringArtwork from "./SteeringArtwork.vue";
import { componentBadge } from "@/utils/concepts";

const props = defineProps({
  reference: {
    type: Object,
    required: true,
  },
  removable: {
    type: Boolean,
    default: true,
  },
  // Percentage of the mix, shown as a small bar.
  share: {
    type: Number,
    default: null,
  },
  // e.g. "61%": how close the last pick was to this.
  closeness: {
    type: String,
    default: "",
  },
  editableWeight: {
    type: Boolean,
    default: false,
  },
  weight: {
    type: Number,
    default: 1,
  },
});

const emit = defineEmits(["remove", "weight"]);

const expanded = ref(false);
const draftWeight = ref(null);
watch(
  () => props.weight,
  () => {
    draftWeight.value = null;
  },
);

const label = computed(
  () => props.reference.label || props.reference.entity_id || "",
);
const badge = computed(() => {
  const value = componentBadge(props.reference);
  return value.charAt(0).toUpperCase() + value.slice(1);
});
const closenessTitle = "How close the last pick was to this";
const fillPercent = computed(
  () => (((draftWeight.value ?? props.weight) - 0.1) / 1.9) * 100,
);

const commit = () => {
  if (draftWeight.value != null) emit("weight", draftWeight.value);
};
</script>

<style scoped>
@import "./steeringCard.css";

.steeringChip {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 4px;
  max-width: 100%;
  padding: 4px 6px 4px 4px;
  border-radius: var(--radius-full);
  background: rgba(255, 255, 255, 0.07);
  transition: background-color var(--transition-fast);
}

.steeringChip:hover {
  background: rgba(255, 255, 255, 0.11);
}

.steeringChip.expanded {
  border-radius: var(--radius-xl);
  background: rgba(255, 255, 255, 0.11);
}

.chipMain {
  display: flex;
  flex: 1 1 auto;
  align-items: center;
  gap: 10px;
  min-width: 0;
  padding: 0 6px 0 0;
  border: none;
  background: none;
  color: inherit;
  text-align: left;
  font: inherit;
}

button.chipMain {
  cursor: pointer;
}

.steeringChip :deep(.steeringArtwork) {
  box-shadow: none;
}

.chipText {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.chipLabel {
  overflow: hidden;
  color: var(--text-bright);
  font-size: var(--text-sm);
  font-weight: var(--font-bold);
  white-space: nowrap;
  text-overflow: ellipsis;
}

.chipMeta {
  color: var(--text-subdued);
  font-size: var(--text-xs);
}

.shareBar {
  position: relative;
  flex: 0 0 auto;
  width: 34px;
  height: 4px;
  margin-left: 4px;
  overflow: hidden;
  border-radius: var(--radius-full);
  background: rgba(255, 255, 255, 0.18);
}

.shareFill {
  position: absolute;
  inset: 0 auto 0 0;
  border-radius: inherit;
  background: var(--spotify-green);
}

.closeness {
  flex: 0 0 auto;
  color: var(--text-subdued);
  font-size: var(--text-xs);
  font-weight: var(--font-bold);
  font-variant-numeric: tabular-nums;
}

.removeButton {
  display: grid;
  flex: 0 0 auto;
  place-items: center;
  width: 26px;
  height: 26px;
  border: none;
  border-radius: 50%;
  background: transparent;
  color: var(--text-subdued);
  font-size: 1.05rem;
  line-height: 1;
  cursor: pointer;
}

.removeButton:hover {
  background: rgba(255, 255, 255, 0.12);
  color: var(--text-bright);
}

.weightPanel {
  display: flex;
  flex: 1 0 100%;
  align-items: center;
  gap: 12px;
  padding: 6px 10px 6px 6px;
}

.weightLabel,
.weightValue {
  flex: 0 0 auto;
  color: var(--text-subdued);
  font-size: var(--text-xs);
  font-weight: var(--font-semibold);
}

.weightValue {
  min-width: 34px;
  text-align: right;
  font-variant-numeric: tabular-nums;
}
</style>
