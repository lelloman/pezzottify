<template>
  <div ref="progressBar" class="progress-bar" @mousedown="startDrag">
    <div class="track" :style="{ width: '100%' }"></div>
    <div class="progress" :style="{ width: progress * 100 + '%' }"></div>
  </div>
</template>

<script setup>
import { ref, defineProps, defineEmits, onMounted, onUnmounted } from 'vue';

defineProps({
  progress: {
    type: Number,
    default: 0.0,
    validator: (value) => value >= 0.0 && value <= 1.0,
  },
});

const emit = defineEmits(['update:progress', 'update:startDrag', 'update:stopDrag']);

const progressBar = ref(null);
const isDragging = ref(false);

const startDrag = (event) => {
  isDragging.value = true;
  emit('update:startDrag');
  updateProgress(event);
  window.addEventListener('mousemove', onDrag);
  window.addEventListener('mouseup', stopDrag);
};

const onDrag = (event) => {
  if (isDragging.value) {
    updateProgress(event);
  }
};

const stopDrag = (event) => {
  emit('update:stopDrag', event)
  isDragging.value = false;
  window.removeEventListener('mousemove', onDrag);
  window.removeEventListener('mouseup', stopDrag);
};

const updateProgress = (event) => {
  if (progressBar.value) {
    const rect = progressBar.value.getBoundingClientRect();
    const offsetX = event.clientX - rect.left;
    const newProgress = Math.min(Math.max(offsetX / rect.width, 0), 1);
    emit('update:progress', newProgress);
  }
};
onMounted(() => {
  onUnmounted(() => {
    window.removeEventListener('mousemove', onDrag);
    window.removeEventListener('mouseup', stopDrag);
  });
});
</script>

<style scoped>
.progress-bar {
  --idle-height: 4px;
  --hover-height: 8px;
  --transition-duration: 0.3s;
  position: relative;
  width: 100%;
  height: var(--idle-height);
  background-color: #535353;
  cursor: pointer;
  border-radius: calc(var(--idle-height) / 2);
  opacity: 0.7;
  transition: height var(--transition-duration) ease, opacity var(--transition-duration) ease;
}

.progress-bar:hover {
  opacity: 1;
  height: var(--hover-height);
  border-radius: calc(var(--hover-height) / 2);
  transition: height var(--transition-duration) ease, opacity var(--transition-duration) ease;
}

.track {
  position: absolute;
  height: 100%;
  background-color: #535353;
  border-radius: var(--idle-height);
}

.progress-bar:hover .track {
  border-radius: calc(var(--hover-height) / 2);
}

.progress {
  position: absolute;
  height: 100%;
  background-color: #aaa;
  border-radius: var(--idle-height);
  transition: background-color var(--transition-duration) ease;
}

.progress-bar:hover .progress {
  background-color: var(--accent-color);
  transition: background-color var(--transition-duration) ease;
  border-radius: calc(var(--hover-height) / 2);
}
</style>
