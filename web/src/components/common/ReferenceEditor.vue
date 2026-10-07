<template>
  <section class="referenceSection">
    <div class="referenceHeader">
      <h3>{{ title }}</h3>
      <button
        v-if="allowManualAdd"
        type="button"
        :aria-label="`Add ${title.toLowerCase()} reference`"
        @click="addReference"
      >
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M12 5v14M5 12h14" />
        </svg>
      </button>
    </div>
    <p v-if="!modelValue.length && emptyText" class="referenceEmpty">
      {{ emptyText }}
    </p>
    <div
      v-for="(reference, index) in modelValue"
      :key="index"
      class="referenceRow"
    >
      <select
        aria-label="Reference type"
        :value="reference.entity_type"
        :disabled="Boolean(reference.label)"
        @change="updateReference(index, { entity_type: $event.target.value })"
      >
        <option v-for="type in ENTITY_TYPES" :key="type" :value="type">
          {{ type }}
        </option>
        <option v-if="reference.entity_type === 'concept'" value="concept">
          concept
        </option>
      </select>
      <span
        v-if="reference.label"
        class="referenceLabel"
        :title="reference.entity_id"
      >
        {{ reference.label }}
      </span>
      <input
        v-else
        aria-label="Reference ID"
        :value="reference.entity_id"
        placeholder="ID"
        @input="updateReference(index, { entity_id: $event.target.value })"
      />
      <input
        type="number"
        min="0"
        step="0.25"
        :value="reference.weight ?? 1"
        :aria-label="`Weight of ${reference.label || reference.entity_id || 'reference'}`"
        @input="
          updateReference(index, { weight: Number($event.target.value) || 1 })
        "
      />
      <button
        type="button"
        :aria-label="`Remove ${reference.label || reference.entity_id || 'reference'}`"
        @click="removeReference(index)"
      >
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="m6 6 12 12M18 6 6 18" />
        </svg>
      </button>
    </div>
  </section>
</template>

<script setup>
const ENTITY_TYPES = ["track", "album", "artist"];

const props = defineProps({
  title: {
    type: String,
    required: true,
  },
  modelValue: {
    type: Array,
    required: true,
  },
  // Free-form ID entry. Pickers that add labelled references can turn it off.
  allowManualAdd: {
    type: Boolean,
    default: true,
  },
  emptyText: {
    type: String,
    default: "",
  },
});

const emit = defineEmits(["update:modelValue"]);

const addReference = () => {
  emit("update:modelValue", [
    ...props.modelValue,
    { entity_type: "track", entity_id: "", weight: 1 },
  ]);
};

const updateReference = (index, patch) => {
  emit(
    "update:modelValue",
    props.modelValue.map((reference, itemIndex) =>
      itemIndex === index ? { ...reference, ...patch } : reference,
    ),
  );
};

const removeReference = (index) => {
  emit(
    "update:modelValue",
    props.modelValue.filter((_, itemIndex) => itemIndex !== index),
  );
};
</script>

<style scoped>
.referenceSection {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.referenceHeader {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.referenceHeader h3 {
  margin: 0;
  font-size: 18px;
  font-weight: 700;
}

.referenceEmpty {
  margin: 0;
  color: var(--text-subdued);
  font-size: 0.85rem;
}

.referenceRow {
  display: grid;
  grid-template-columns: 82px minmax(0, 1fr) 58px 32px;
  align-items: center;
  gap: 8px;
}

.referenceLabel {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text-bright);
}

input,
select,
button {
  min-height: 34px;
}

input,
select {
  min-width: 0;
  width: 100%;
  border: 1px solid #727272;
  border-radius: 4px;
  background: #333;
  color: var(--text-bright);
  padding: 6px;
  font: inherit;
  font-size: 13px;
  outline: none;
}

input:focus,
select:focus {
  border-color: #fff;
  box-shadow: inset 0 0 0 1px #fff;
}

button {
  display: grid;
  place-items: center;
  width: 32px;
  border: 0;
  border-radius: 50%;
  background: transparent;
  color: var(--text-subdued);
  padding: 0;
  cursor: pointer;
}

button:hover {
  background: var(--bg-press);
  color: var(--text-bright);
}
button:focus-visible {
  outline: 2px solid white;
  outline-offset: 2px;
}
button svg {
  width: 20px;
  height: 20px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.8;
}
</style>
