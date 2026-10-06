<template>
  <div
    ref="progressBar"
    class="progress-bar"
    :class="{ dragging: isDragging }"
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
import { ref, onUnmounted } from "vue";

const props = defineProps({
  progress: {
    type: Number,
    default: 0.0,
    validator: (value) => value >= 0.0 && value <= 1.0,
  },
});

const emit = defineEmits([
  "update:progress",
  "update:startDrag",
  "update:stopDrag",
]);

const progressBar = ref(null);
const isDragging = ref(false);

const startDrag = (event) => {
  event.preventDefault();
  isDragging.value = true;
  emit("update:startDrag");
  updateProgress(event);

  // Support both mouse and touch events
  window.addEventListener("mousemove", onDrag);
  window.addEventListener("mouseup", stopDrag);
  window.addEventListener("touchmove", onDrag, { passive: false });
  window.addEventListener("touchend", stopDrag);
};

const onDrag = (event) => {
  if (isDragging.value) {
    event.preventDefault();
    updateProgress(event);
  }
};

const stopDrag = (event) => {
  emit("update:stopDrag", event);
  isDragging.value = false;
  window.removeEventListener("mousemove", onDrag);
  window.removeEventListener("mouseup", stopDrag);
  window.removeEventListener("touchmove", onDrag);
  window.removeEventListener("touchend", stopDrag);
};

const updateProgress = (event) => {
  if (progressBar.value) {
    const rect = progressBar.value.getBoundingClientRect();
    // Get clientX from either mouse or touch event
    const clientX = event.touches ? event.touches[0].clientX : event.clientX;
    const offsetX = clientX - rect.left;
    const newProgress = Math.min(Math.max(offsetX / rect.width, 0), 1);
    emit("update:progress", newProgress);
  }
};

const handleKeyDown = (event) => {
  const step = event.shiftKey ? 0.1 : 0.05; // Larger step with Shift key
  let newProgress = props.progress;

  switch (event.key) {
    case "ArrowLeft":
    case "ArrowDown":
      event.preventDefault();
      newProgress = Math.max(0, props.progress - step);
      break;
    case "ArrowRight":
    case "ArrowUp":
      event.preventDefault();
      newProgress = Math.min(1, props.progress + step);
      break;
    case "Home":
      event.preventDefault();
      newProgress = 0;
      break;
    case "End":
      event.preventDefault();
      newProgress = 1;
      break;
    default:
      return;
  }

  emit("update:startDrag");
  emit("update:progress", newProgress);
  emit("update:stopDrag", event);
};
onUnmounted(() => {
  window.removeEventListener("mousemove", onDrag);
  window.removeEventListener("mouseup", stopDrag);
  window.removeEventListener("touchmove", onDrag);
  window.removeEventListener("touchend", stopDrag);
});
</script>

<style scoped>
.progress-bar {
  position: relative;
  width: 100%;
  height: 16px;
  cursor: pointer;
  touch-action: none;
}
.track,
.progress {
  position: absolute;
  left: 0;
  top: 6px;
  height: 4px;
  border-radius: 999px;
}
.track {
  width: 100%;
  background: #ffffff4d;
}
.progress {
  background: var(--text-base);
}
.progress-bar:hover .progress,
.progress-bar:focus-visible .progress,
.dragging .progress {
  background: var(--spotify-green);
}
.thumb {
  position: absolute;
  right: -6px;
  top: 50%;
  transform: translateY(-50%);
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: white;
  opacity: 0;
  pointer-events: none;
}
.progress-bar:hover .thumb,
.progress-bar:focus-visible .thumb,
.dragging .thumb {
  opacity: 1;
}
.progress-bar:focus-visible {
  outline: 2px solid var(--spotify-green);
  outline-offset: 2px;
  border-radius: 4px;
}
</style>
