<template>
  <ModalDialog :closeOnEsc="true" :isOpen="props.isOpen" :closeCallback="props.closeCallback">
    <div class="modalContent">
      <h1>{{ props.title }}</h1>
      <p>
        <slot name="message"></slot>
      </p>
      <div class="modal-buttons">
        <div class="button" @click="props.closeCallback">{{ props.negativeButtonText }}</div>
        <div class="button positiveButton" @click="props.positiveButtonCallback">{{ props.positiveButtonText }}</div>
      </div>
    </div>
  </ModalDialog>
</template>

<script setup>
import ModalDialog from '@/components/common/ModalDialog.vue';

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
.modalContent h1 {
  margin: 0;
  font-size: 28px;
}

.modalContent p {
  margin: 8px 0;
  font-size: 14px;
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
  transition: scale 0.3s ease, background-color 0.3s ease;
}

.button:hover {
  background-color: rgba(0, 0, 0, 0.2);
  scale: 1.05;
  transition: scale 0.3s ease, background-color 0.3s ease;
}

.button:active {
  background-color: rgba(0, 0, 0, 0.3);
  scale: 0.95;
  transition: scale 0.3s ease, background-color 0.3s ease;
}

.positiveButton {
  background-color: var(--accent-color) !important;
  color: white;
}
</style>
