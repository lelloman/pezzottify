<template>
  <div v-if="isOpen" ref="menu" class="container" :style="{ top: `${adjustedY}px`, left: `${adjustedX}px` }">
    <div v-for="item in items" :key="item.name" class="contextMenuItem"
      @mouseenter="item.subMenu && startHoverTimer(item)" @mouseleave="clearHoverTimer"
      @click.stop="executeItemAction(item)">

      <component :is="item.icon" v-if="item.icon" class="smallIcon" style="fill: #ddd" />

      <span>{{ item.name }}</span>

      <ChevronRight v-if="item.subMenu" class="smallIcon" style="fill: #eee;" />

      <div v-if="isSubMenuOpen && currentSubMenu === item" class="subMenu"
        :style="{ top: `${subMenuY}px`, left: `${subMenuX}px` }">
        <div class="contextMenuItem" v-for="subItem in item.subMenu()" :key="subItem.name"
          @click.stop="executeSubItemAction(subItem)">
          {{ subItem.name }}
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { watch, computed, useTemplateRef, ref, defineExpose } from 'vue';
import ChevronRight from '@/components/icons/ChevronRight.vue';

defineProps({
  items: {
    type: Array,
    required: true
  }
});

const menu = useTemplateRef('menu');

const contextData = ref({
  x: 0,
  y: 0,
  isOpen: false,
});

const isOpen = computed(() => contextData.value.isOpen);

const executeItemAction = (item) => {
  const hasSubmenu = item.subMenu !== undefined;
  console.log("Executing item '" + item.name + "' action (hasSubmenu: " + hasSubmenu + ")");
  if (hasSubmenu) return;

  item.action();
  contextData.value.isOpen = false;
};

const executeSubItemAction = (subItem) => {
  const hasSubmenu = subItem.subMenu !== undefined;
  console.log("Executing subItem '" + subItem.name + "' action " + (hasSubmenu ? " (hasSubmenu)" : ""));
  if (hasSubmenu) return;

  subItem.action();
  isSubMenuOpen.value = false;
  contextData.value.isOpen = false;
};

watch(() => contextData.value.isOpen, (isOpen) => {
  if (isOpen) {
    window.addEventListener('click', handleClickOutside, true);
    window.addEventListener('contextmenu', handleClickOutside, true);
    window.addEventListener('keydown', handleKeydown, true);
  } else {
    window.removeEventListener('click', handleClickOutside, true);
    window.removeEventListener('contextmenu', handleClickOutside, true);
    window.removeEventListener('keydown', handleKeydown, true);
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

const handleClickOutside = (event) => {
  event.preventDefault();
  const isOutside = !menu.value.contains(event.target);
  if (isOutside) {
    contextData.value.isOpen = false;
    event.stopImmediatePropagation();
  }
};

const handleKeydown = (event) => {
  if (event.key === 'Escape') {
    contextData.value.isOpen = false;
  }
};

const openMenu = (event) => {
  contextData.value = {
    x: event.clientX,
    y: event.clientY,
    isOpen: true,
  };
};

const isSubMenuOpen = ref(false);
const subMenuX = ref(0);
const subMenuY = ref(0);
let hoverTimer = null;
let currentSubMenu = null;

const startHoverTimer = (item) => {
  hoverTimer = setTimeout(() => {
    isSubMenuOpen.value = true;
    currentSubMenu = item;
    subMenuX.value = menu.value.getBoundingClientRect().right;
    subMenuY.value = menu.value.getBoundingClientRect().top;
  }, 400);
};

const clearHoverTimer = () => {
  clearTimeout(hoverTimer);
  isSubMenuOpen.value = false;
  currentSubMenu = null;
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
  display: flex;
  flex-direction: row;
  height: 50px;
  cursor: pointer;
  align-items: center;
  font-size: 14px;
  padding: 0 8px;
}

.contextMenuItem span {
  flex: 1;
  padding: 0 8px;
}

.contextMenuItem:hover {
  background-color: #222;
}

.subMenu {
  z-index: 1001;
  position: fixed;
  width: 200px;
  border: 1px solid #ccc;
  background-color: #151515;
  z-index: 1001;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
</style>
