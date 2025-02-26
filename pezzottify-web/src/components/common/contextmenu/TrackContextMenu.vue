<template>
  <div v-if="isOpen" ref="menu" class="container" :style="{ top: `${adjustedY}px`, left: `${adjustedX}px` }">

    <div class="contextMenuItem" @click="handleClick('play')">
      Add to playlist
    </div>

    <div class="contextMenuItem" @click="handleClick('add')">
      Add to queue
    </div>
  </div>
</template>

<script setup>
import { watch, computed, useTemplateRef, ref, defineExpose } from 'vue'

const menu = useTemplateRef('menu');

const contextData = ref({
  track: null,
  x: 0,
  y: 0,
  isOpen: false,
});

const isOpen = computed(() => contextData.value.isOpen);

watch(() => contextData.value.isOpen, (isOpen) => {
  console.log("TrackContextMenu isOpen changed to: " + isOpen);
  if (isOpen) {
    window.addEventListener('click', handleClickOutside, true); // Use capture mode
    window.addEventListener('contextmenu', handleClickOutside, true); // Use capture mode for right-click
    window.addEventListener('keydown', handleKeydown, true); // Use capture mode
  } else {
    window.removeEventListener('click', handleClickOutside, true); // Use capture mode
    window.removeEventListener('contextmenu', handleClickOutside, true); // Use capture mode for right-click
    window.removeEventListener('keydown', handleKeydown, true); // Use capture mode
  }
});

const adjustedX = computed(() => {
  if (!menu.value) return;

  const menuWidth = menu.value.getBoundingClientRect().width;
  const viewportWidth = window.innerWidth;
  const x = contextData.value.x;
  return x + menuWidth > viewportWidth ? x - menuWidth : x;
});

const adjustedY = computed(() => {
  if (!menu.value) return;

  const menuHeight = menu.value.getBoundingClientRect().height;
  const viewportHeight = window.innerHeight;
  const y = contextData.value.y;
  return y + menuHeight > viewportHeight ? viewportHeight - menuHeight : y;
});

const handleClick = (option) => {
  this.$emit('select', option);
  this.closeMenu();
};

const handleClickOutside = (event) => {
  event.preventDefault(); // Prevent the default context menu
  event.stopImmediatePropagation();
  const isOutside = !menu.value.contains(event.target);
  console.log("TrackContextMenu handleClickOutside " + isOutside);
  // Check if the click is outside the context menu
  if (isOutside) {
    // emit a "close" event
    contextData.value = {
      ...contextData.value,
      isOpen: false,
    };
  }
};

const handleKeydown = (event) => {
  // Close the menu if the Escape key is pressed
  if (event.key === 'Escape') {
    contextData.value = {
      ...contextData.value,
      isOpen: false,
    };
  }
};

const openMenu = (event, track) => {
  contextData.value = {
    track: track,
    x: event.clientX,
    y: event.clientY,
    isOpen: true,
  };
  console.log("TrackContextMenu openMenu");
  console.log(contextData.value);
};

defineExpose({
  openMenu,
});

</script>

<style scoped>
.container {
  position: fixed;
  width: 220px;
  border: 1px solid #ccc;
  background-color: #151515;
  z-index: 1000;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.contextMenuItem {
  padding: 8px;
  height: 50px;
  cursor: pointer;
  align-content: center;
  font-size: 14px;
  padding: 0 16px;
}

.contextMenuItem:hover {
  background-color: #222;
}
</style>
