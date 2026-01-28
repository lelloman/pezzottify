<template>
  <div class="ingestion-section" :class="{ collapsed: !isOpen }">
    <button class="section-header" @click="toggle">
      <span class="section-title">{{ title }}</span>
      <span v-if="badge" class="section-badge">{{ badge }}</span>
      <span class="toggle-icon">{{ isOpen ? '&#9650;' : '&#9660;' }}</span>
    </button>
    <div v-show="isOpen" class="section-content">
      <slot></slot>
    </div>
  </div>
</template>

<script setup>
import { ref } from "vue";

const props = defineProps({
  title: {
    type: String,
    required: true,
  },
  badge: {
    type: String,
    default: null,
  },
  defaultOpen: {
    type: Boolean,
    default: true,
  },
});

const isOpen = ref(props.defaultOpen);

function toggle() {
  isOpen.value = !isOpen.value;
}
</script>

<style scoped>
.ingestion-section {
  margin-bottom: 8px;
  border: 1px solid var(--border-default);
  border-radius: 4px;
  overflow: hidden;
}

.section-header {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 12px;
  background: var(--bg-highlight);
  border: none;
  cursor: pointer;
  text-align: left;
  font-size: 14px;
  color: var(--text-base);
}

.section-header:hover {
  background: var(--bg-press);
}

.section-title {
  flex: 1;
  font-weight: 500;
  color: var(--text-base);
}

.section-badge {
  padding: 2px 8px;
  background: var(--bg-press);
  border-radius: 10px;
  font-size: 12px;
  color: var(--text-subdued);
}

.toggle-icon {
  font-size: 10px;
  color: var(--text-subtle);
}

.section-content {
  padding: 12px;
  background: var(--bg-elevated);
}
</style>
