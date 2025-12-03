<template>
  <div
    ref="progressBar"
    class="progress-bar"
    @mousedown="startDrag"
    @touchstart="startDrag"
    role="slider"
    :aria-valuemin="0"
    :aria-valuemax="100"
    :aria-valuenow="Math.round(progress * 100)"
    tabindex="0"
    @keydown="handleKeyDown"
  >
    <div class="track"></div>
    <div class="progress" :style="{ width: progress * 100 + '%' }">
      <div class="thumb"></div>
    </div>
  </div>
</template>

<script setup>
import { ref, defineProps, defineEmits, onMounted, onUnmounted } from 'vue';

const props = defineProps({
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
  event.preventDefault();
  isDragging.value = true;
  emit('update:startDrag');
  updateProgress(event);

  // Support both mouse and touch events
  window.addEventListener('mousemove', onDrag);
  window.addEventListener('mouseup', stopDrag);
  window.addEventListener('touchmove', onDrag, { passive: false });
  window.addEventListener('touchend', stopDrag);
};

const onDrag = (event) => {
  if (isDragging.value) {
    event.preventDefault();
    updateProgress(event);
  }
};

const stopDrag = (event) => {
  emit('update:stopDrag', event)
  isDragging.value = false;
  window.removeEventListener('mousemove', onDrag);
  window.removeEventListener('mouseup', stopDrag);
  window.removeEventListener('touchmove', onDrag);
  window.removeEventListener('touchend', stopDrag);
};

const updateProgress = (event) => {
  if (progressBar.value) {
    const rect = progressBar.value.getBoundingClientRect();
    // Get clientX from either mouse or touch event
    const clientX = event.touches ? event.touches[0].clientX : event.clientX;
    const offsetX = clientX - rect.left;
    const newProgress = Math.min(Math.max(offsetX / rect.width, 0), 1);
    emit('update:progress', newProgress);
  }
};

const handleKeyDown = (event) => {
  const step = event.shiftKey ? 0.1 : 0.05; // Larger step with Shift key
  let newProgress = props.progress;

  switch (event.key) {
    case 'ArrowLeft':
    case 'ArrowDown':
      event.preventDefault();
      newProgress = Math.max(0, props.progress - step);
      break;
    case 'ArrowRight':
    case 'ArrowUp':
      event.preventDefault();
      newProgress = Math.min(1, props.progress + step);
      break;
    case 'Home':
      event.preventDefault();
      newProgress = 0;
      break;
    case 'End':
      event.preventDefault();
      newProgress = 1;
      break;
    default:
      return;
  }

  emit('update:startDrag');
  emit('update:progress', newProgress);
  emit('update:stopDrag', event);
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
  --thumb-size: 12px;
  position: relative;
  width: 100%;
  height: var(--idle-height);
  cursor: pointer;
  border-radius: calc(var(--idle-height) / 2);
  transition: height var(--transition-base);
  touch-action: none;
}

.progress-bar:hover,
.progress-bar:focus-visible {
  height: var(--hover-height);
}

.progress-bar:focus-visible {
  outline: 2px solid var(--spotify-green);
  outline-offset: 4px;
  border-radius: var(--radius-sm);
}

.track {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  background-color: var(--bg-press);
  border-radius: inherit;
  transition: background-color var(--transition-base);
}

.progress {
  position: absolute;
  top: 0;
  left: 0;
  height: 100%;
  background-color: var(--text-subdued);
  border-radius: inherit;
  transition: background-color var(--transition-base), width var(--transition-fast);
  display: flex;
  align-items: center;
  justify-content: flex-end;
}

.progress-bar:hover .progress,
.progress-bar:focus-visible .progress {
  background-color: var(--spotify-green);
}

.thumb {
  width: var(--thumb-size);
  height: var(--thumb-size);
  background-color: var(--text-base);
  border-radius: var(--radius-full);
  opacity: 0;
  transform: scale(0);
  transition: opacity var(--transition-fast), transform var(--transition-fast);
  box-shadow: var(--shadow-md);
  pointer-events: none;
}

.progress-bar:hover .thumb,
.progress-bar:focus-visible .thumb {
  opacity: 1;
  transform: scale(1);
}

/* Show thumb when dragging even if not hovering */
.progress-bar:active .thumb {
  opacity: 1;
  transform: scale(1);
}

/* Increase hit area for better touch support */
.progress-bar::before {
  content: '';
  position: absolute;
  top: -8px;
  bottom: -8px;
  left: 0;
  right: 0;
  cursor: pointer;
}
</style>
