<template>
  <Transition>
    <div v-if="isOpen" class="modal-overlay">
      <div class="modal">
        <slot></slot>
      </div>
    </div>
  </Transition>
</template>

<script setup>
import { defineProps, onUnmounted, watch } from "vue";

const props = defineProps({
  isOpen: {
    type: Boolean,
    required: true,
  },
  closeCallback: {
    type: Function,
    required: true,
  },
  closeOnEsc: {
    type: Boolean,
    default: false,
  },
});

const closeOnEsc = (e) => {
  console.log("Modal dialog close on Esc listener");
  if (e.key === "Escape") {
    props.closeCallback();
  }
};

watch(
  () => props.isOpen,
  (isOpen) => {
    if (isOpen && props.closeOnEsc === true) {
      window.addEventListener("keydown", closeOnEsc);
    } else {
      window.removeEventListener("keydown", closeOnEsc);
    }
  },
  { immediate: true },
);

// Removes the esc listener when the component is unmounted just in case?
onUnmounted(() => {
  window.removeEventListener("keydown", closeOnEsc);
});
</script>

<style>
.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  background: rgba(0, 0, 0, 0.7);
  display: flex;
  justify-content: center;
  align-items: center;
  z-index: 1000;
}

.modal {
  background: var(--bg-elevated);
  color: var(--text-base);
  padding: 20px;
  border-radius: 8px;
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.5);
}

.v-enter-active,
.v-leave-active {
  transition: opacity 0.3s ease;
}

.v-enter-from,
.v-leave-to {
  opacity: 0;
}
</style>
