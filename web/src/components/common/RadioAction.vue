<template>
  <details ref="menu" class="radioAction" @keydown.esc.stop="closeAndFocus">
    <summary ref="trigger" aria-label="Radio" title="Radio">
      <RadioIcon aria-hidden="true" />
    </summary>
    <div class="radioMenu">
      <button type="button" @click="select('start')">Start radio</button>
      <button type="button" @click="select('customize')">
        Customize radio
      </button>
    </div>
  </details>
</template>

<script setup>
import { ref, onMounted, onBeforeUnmount, onDeactivated } from "vue";
import RadioIcon from "@/components/icons/RadioIcon.vue";
const emit = defineEmits(["start", "customize"]);
const menu = ref(null);
const trigger = ref(null);
const close = () => {
  if (menu.value) menu.value.open = false;
};
const closeAndFocus = () => {
  close();
  trigger.value?.focus();
};
const select = (action) => {
  closeAndFocus();
  emit(action);
};
const outside = (event) => {
  if (!menu.value?.contains(event.target)) close();
};
onMounted(() => document.addEventListener("pointerdown", outside));
onBeforeUnmount(() => document.removeEventListener("pointerdown", outside));
onDeactivated(close);
</script>

<style scoped>
.radioAction {
  position: relative;
}
summary {
  display: grid;
  place-items: center;
  width: 32px;
  height: 56px;
  list-style: none;
  color: var(--text-subdued);
  cursor: pointer;
  border-radius: 50%;
  transition:
    color 150ms cubic-bezier(0.3, 0, 0, 1),
    transform 150ms cubic-bezier(0.3, 0, 0, 1);
}
summary::-webkit-details-marker {
  display: none;
}
summary:hover,
.radioAction[open] summary {
  color: var(--text-base);
}
summary svg {
  width: 32px;
  height: 32px;
}
.radioMenu {
  position: absolute;
  top: calc(100% + 8px);
  left: 0;
  z-index: 40;
  width: 190px;
  max-width: calc(100cqw - 160px);
  padding: 6px;
  background: #282828;
  border-radius: 6px;
  box-shadow: 0 12px 36px #0008;
}
.radioMenu button {
  display: block;
  width: 100%;
  padding: 12px;
  border: 0;
  border-radius: 3px;
  background: none;
  color: var(--text-base);
  text-align: left;
  font: inherit;
  cursor: pointer;
}
.radioMenu button:hover,
.radioMenu button:focus-visible {
  background: #ffffff12;
}
@media (hover: hover) {
  summary:hover {
    background: transparent;
    transform: scale(1.04);
    transition-duration: 50ms;
  }
}
summary:active {
  background: transparent;
  transform: scale(1);
}
summary:focus-visible,
button:focus-visible {
  outline: 2px solid var(--spotify-green);
  outline-offset: 3px;
}
@media (prefers-reduced-motion: reduce) {
  button,
  summary {
    transition: none;
  }
}
</style>
