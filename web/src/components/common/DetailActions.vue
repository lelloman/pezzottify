<template>
  <div class="detailActions">
    <button
      type="button"
      class="primaryPlay"
      :aria-label="playLabel"
      :title="playLabel"
      :disabled="disabled"
      @click="$emit('play')"
    >
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <path d="M7 3v18l15-9z" />
      </svg>
    </button>
    <button
      v-if="showSave"
      type="button"
      class="saveButton"
      :class="{ saved }"
      :aria-pressed="saved"
      :aria-label="saved ? 'Remove from library' : 'Save to library'"
      :title="saved ? 'Remove from library' : 'Save to library'"
      @click="$emit('save')"
    >
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <circle cx="12" cy="12" r="9" />
        <path v-if="saved" d="m7 12 3 3 7-7" />
        <path v-else d="M7 12h10M12 7v10" />
      </svg>
    </button>
    <div v-if="$slots.secondary" class="secondaryActions">
      <slot name="secondary" />
    </div>
    <details
      v-if="$slots.more || $slots.secondary"
      ref="more"
      class="moreActions"
      :class="{ responsiveOnly: !$slots.more }"
      @keydown.esc="close"
    >
      <summary aria-label="More actions" title="More actions">•••</summary>
      <div class="morePanel" @click.capture="handleAction">
        <div v-if="$slots.secondary" class="secondaryOverflow">
          <slot name="secondary" />
        </div>
        <slot name="more" />
      </div>
    </details>
  </div>
</template>
<script setup>
import { ref, onMounted, onBeforeUnmount, onDeactivated } from "vue";
defineProps({
  playLabel: { type: String, default: "Play" },
  disabled: Boolean,
  showSave: Boolean,
  saved: Boolean,
});
defineEmits(["play", "save"]);
const more = ref(null);
const close = () => {
  if (more.value) more.value.open = false;
};
const outside = (e) => {
  if (more.value && !more.value.contains(e.target)) close();
};
const handleAction = (e) => {
  if (e.target.closest("button")) close();
};
onMounted(() => document.addEventListener("pointerdown", outside));
onBeforeUnmount(() => document.removeEventListener("pointerdown", outside));
onDeactivated(close);
</script>
<style scoped>
.detailActions {
  display: flex;
  align-items: center;
  gap: 24px;
}
button,
summary {
  cursor: pointer;
}
.primaryPlay {
  display: grid;
  place-items: center;
  width: 56px;
  height: 56px;
  flex-shrink: 0;
  padding: 16px;
  border: 0;
  border-radius: 50%;
  background: var(--spotify-green);
  color: #071108;
  transition: transform 0.15s;
}
.primaryPlay svg {
  width: 100%;
  height: 100%;
  fill: currentColor;
}
.primaryPlay:hover:not(:disabled) {
  transform: scale(1.06);
  background: var(--spotify-green-hover);
}
.primaryPlay:disabled {
  opacity: 0.45;
  cursor: wait;
}
.saveButton {
  padding: 0;
  width: 32px;
  height: 32px;
  background: none;
  border: 0;
  color: var(--text-subdued);
}
.saveButton svg {
  width: 100%;
  height: 100%;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.8;
  stroke-linecap: round;
  stroke-linejoin: round;
}
.saveButton.saved {
  color: var(--spotify-green);
}
.saveButton:hover {
  color: var(--text-base);
}
.moreActions {
  position: relative;
}
summary {
  list-style: none;
  color: var(--text-subdued);
  font-size: 22px;
  letter-spacing: 3px;
  padding: 10px;
  line-height: 1;
}
summary::-webkit-details-marker {
  display: none;
}
summary:hover {
  color: var(--text-base);
}
.morePanel {
  position: absolute;
  top: 100%;
  left: 0;
  z-index: 40;
  background: #282828;
  box-shadow: 0 12px 36px #0008;
  padding: 6px;
  border-radius: 6px;
  width: 240px;
  max-width: calc(100cqw - 152px);
}
.morePanel :deep(button) {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  border: 0;
  border-radius: 3px;
  padding: 12px;
  background: transparent;
  color: #eee;
  text-align: left;
  font: inherit;
  cursor: pointer;
  white-space: normal;
}
.morePanel :deep(button:hover) {
  background: #ffffff12;
}
.morePanel :deep(svg) {
  width: 20px;
  height: 20px;
  flex-shrink: 0;
}
.secondaryActions :deep(svg:not([fill="none"])),
.morePanel :deep(svg:not([fill="none"])) {
  fill: currentColor;
}
.secondaryActions {
  display: flex;
  align-items: center;
  gap: 20px;
}
.secondaryActions :deep(button) {
  display: grid;
  place-items: center;
  padding: 0;
  width: 32px;
  height: 40px;
  min-height: 0;
  border: 0;
  background: none;
  color: var(--text-subdued);
  cursor: pointer;
}
.secondaryActions :deep(button:hover) {
  color: var(--text-base);
  background: none;
}
.secondaryActions :deep(svg) {
  width: 28px;
  height: 28px;
}
.secondaryActions :deep(.actionLabel) {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip-path: inset(50%);
  white-space: nowrap;
}
.secondaryOverflow,
.responsiveOnly {
  display: none;
}
@container (max-width:560px) {
  .secondaryActions {
    display: none;
  }
  .secondaryOverflow,
  .responsiveOnly {
    display: block;
  }
}
</style>
