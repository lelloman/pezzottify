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
    <slot name="inline" />
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
      <summary aria-label="More actions" title="More actions">
        <svg
          viewBox="0 0 24 24"
          width="32"
          height="32"
          fill="currentColor"
          aria-hidden="true"
        >
          <circle cx="4" cy="12" r="2" />
          <circle cx="12" cy="12" r="2" />
          <circle cx="20" cy="12" r="2" />
        </svg>
      </summary>
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
  position: relative;
  display: flex;
  align-items: center;
  gap: 24px;
  flex-wrap: wrap;
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
  background: #1ed760;
  color: #000;
  transition:
    color 150ms cubic-bezier(0.3, 0, 0, 1),
    transform 150ms cubic-bezier(0.3, 0, 0, 1);
}
.primaryPlay svg {
  width: 100%;
  height: 100%;
  fill: currentColor;
}
.primaryPlay:hover:not(:disabled) {
  transform: scale(1.04);
  background: #3be477;
}
.primaryPlay:disabled {
  opacity: 0.45;
  cursor: wait;
}
.saveButton {
  padding: 12px 0;
  width: 32px;
  height: 56px;
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
  color: #1ed760;
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
  display: grid;
  place-items: center;
  width: 32px;
  height: 56px;
  padding: 0;
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
  background: var(--menu-background);
  box-shadow: var(--shadow-menu);
  padding: 4px;
  border-radius: 4px;
  width: 240px;
  max-width: calc(100cqw - 152px);
}
.morePanel :deep(button) {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  border: 0;
  border-radius: 2px;
  padding: 8px 12px;
  min-height: 40px;
  background: transparent;
  color: #eee;
  text-align: left;
  font: inherit;
  font-size: var(--text-sm);
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
  gap: 24px;
}
.secondaryActions :deep(button) {
  display: grid;
  place-items: center;
  padding: 0;
  width: 32px;
  height: 56px;
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
  width: 32px;
  height: 32px;
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
  .moreActions {
    position: static;
  }
  .morePanel {
    top: auto;
    bottom: calc(100% + 8px);
    left: 0;
    max-width: 100%;
  }
  .secondaryActions {
    display: none;
  }
  .secondaryOverflow,
  .responsiveOnly {
    display: block;
  }
}
.saveButton,
summary,
.secondaryActions :deep(button) {
  border-radius: 50%;
  transition:
    color 150ms cubic-bezier(0.3, 0, 0, 1),
    transform 150ms cubic-bezier(0.3, 0, 0, 1);
}
@media (hover: hover) {
  .saveButton:hover,
  summary:hover,
  .secondaryActions :deep(button:hover) {
    background: transparent;
    transform: scale(1.04);
    transition-duration: 50ms;
  }
  .saveButton.saved:hover {
    color: #1ed760;
  }
}
button:focus-visible,
summary:focus-visible,
.secondaryActions :deep(button:focus-visible),
.morePanel :deep(button:focus-visible) {
  outline: 2px solid var(--spotify-green);
  outline-offset: 3px;
}
.saveButton:active,
summary:active,
.secondaryActions :deep(button:active) {
  background: transparent;
  transform: scale(1);
}
.primaryPlay:active:not(:disabled) {
  transform: scale(1);
}
@media (prefers-reduced-motion: reduce) {
  button,
  summary {
    transition: none;
  }
}
</style>
