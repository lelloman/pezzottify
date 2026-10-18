<template>
  <dialog ref="dialog" :aria-label="title" @cancel.prevent="closeCallback">
    <h3>{{ title }}</h3>
    <p><slot name="message" /></p>
    <button autofocus @click="closeCallback">{{ negativeButtonText }}</button>
    <button @click="positiveButtonCallback">{{ positiveButtonText }}</button>
  </dialog>
</template>
<script setup>
import { ref, watch, nextTick } from "vue";
const props = defineProps({
  isOpen: Boolean,
  title: { type: String, required: true },
  closeCallback: { type: Function, required: true },
  positiveButtonCallback: { type: Function, required: true },
  positiveButtonText: { type: String, required: true },
  negativeButtonText: { type: String, default: "Cancel" },
});
const dialog = ref(null);
watch(
  () => props.isOpen,
  async (open) => {
    await nextTick();
    if (open && props.isOpen && !dialog.value?.open) dialog.value?.showModal();
    else if (!props.isOpen) dialog.value?.close();
  },
  { immediate: true },
);
</script>
<style scoped>
:focus-visible {
  outline: 2px solid #72d8ff;
  outline-offset: 3px;
}
dialog {
  max-width: min(32rem, 90vw);
  overflow-wrap: anywhere;
  background: var(--bg-elevated-base);
  color: var(--text-base);
  border: 1px solid var(--border-subdued);
  border-radius: 0.5rem;
}
dialog::backdrop {
  background: #000a;
}
button {
  font: inherit;
  padding: 0.5rem;
  margin: 0.5rem;
}
</style>
