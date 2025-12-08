<template>
  <ModalDialog
    :closeOnEsc="true"
    :isOpen="props.isOpen"
    :closeCallback="props.closeCallback"
  >
    <div class="modalContent" style="color: #1a1a1a !important">
      <h1 style="color: #1a1a1a !important">{{ props.title }}</h1>
      <p style="color: #333333 !important">
        <slot name="message"></slot>
      </p>
      <div class="modal-buttons">
        <div class="button" @click="props.closeCallback">
          {{ props.negativeButtonText }}
        </div>
        <div
          class="button positiveButton"
          @click="props.positiveButtonCallback"
        >
          {{ props.positiveButtonText }}
        </div>
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
  color: #1a1a1a;
}

.modalContent h1 {
  margin: 0;
  font-size: 28px;
  color: #1a1a1a;
}

.modalContent p {
  margin: 8px 0;
  font-size: 14px;
  color: #333;
}

.modal-buttons {
  display: flex;
  flex-direction: row;
  gap: 24px;
  justify-content: flex-end;
  padding: 16px 0;
}

.button {
  cursor: pointer;
  padding: 8px 16px;
  border-radius: 4px;
  background-color: rgba(0, 0, 0, 0.1);
  transition:
    scale 0.3s ease,
    background-color 0.3s ease;
}

.button:hover {
  background-color: rgba(0, 0, 0, 0.2);
  scale: 1.05;
  transition:
    scale 0.3s ease,
    background-color 0.3s ease;
}

.button:active {
  background-color: rgba(0, 0, 0, 0.3);
  scale: 0.95;
  transition:
    scale 0.3s ease,
    background-color 0.3s ease;
}

.positiveButton {
  background-color: var(
    --accent-color,
    var(--spotify-green, #1db954)
  ) !important;
  color: white;
}
</style>
