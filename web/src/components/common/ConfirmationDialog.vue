<template>
  <ModalDialog
    :closeOnEsc="true"
    :isOpen="props.isOpen"
    :closeCallback="props.closeCallback"
  >
    <div class="modalContent">
      <h1>{{ props.title }}</h1>
      <div class="message">
        <slot name="message"></slot>
      </div>
      <div class="modal-buttons">
        <button
          type="button"
          class="button negativeButton"
          @click="props.closeCallback"
        >
          {{ props.negativeButtonText }}
        </button>
        <button
          type="button"
          class="button positiveButton"
          @click="props.positiveButtonCallback"
        >
          {{ props.positiveButtonText }}
        </button>
      </div>
    </div>
  </ModalDialog>
</template>

<script setup>
import ModalDialog from "@/components/common/ModalDialog.vue";

const props = defineProps({
  isOpen: {
    type: Boolean,
    required: true,
  },
  closeCallback: {
    type: Function,
    required: true,
  },
  positiveButtonCallback: {
    type: Function,
    required: true,
  },
  title: {
    type: String,
    required: true,
  },
  positiveButtonText: {
    type: String,
    default: "Yes",
  },
  negativeButtonText: {
    type: String,
    default: "No",
  },
});
</script>

<style scoped>
.modalContent {
  width: min(420px, calc(100vw - 72px));
  color: var(--text-base);
}
.modalContent h1 {
  margin: 0;
  font-size: 24px;
  line-height: 1.3;
  font-weight: 700;
  color: var(--text-base);
}
.message {
  margin: 16px 0 24px;
  font-size: 14px;
  line-height: 1.6;
  color: var(--text-subdued);
}
.modal-buttons {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
  justify-content: flex-end;
}
.button {
  min-height: 44px;
  padding: 10px 24px;
  border-radius: 999px;
  font: inherit;
  font-size: 14px;
  font-weight: 700;
  cursor: pointer;
  transition:
    background-color 150ms,
    transform 150ms;
}
.negativeButton {
  color: var(--text-base);
  background: transparent;
  border: 1px solid var(--text-subtle);
}
.negativeButton:hover {
  border-color: var(--text-base);
  background: var(--surface-hover);
}
.positiveButton {
  background: var(--spotify-green);
  color: #000;
  border: 1px solid transparent;
}
.positiveButton:hover {
  background: var(--spotify-green-hover);
}
.button:hover {
  transform: scale(1.04);
}
.button:active {
  transform: scale(1);
}
.button:focus-visible {
  outline: 2px solid var(--text-base);
  outline-offset: 3px;
}
@media (prefers-reduced-motion: reduce) {
  .button {
    transition: none;
  }
}
</style>
