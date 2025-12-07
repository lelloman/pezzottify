<template>
  <aside class="adminSidebar">
    <nav class="sidebarNav">
      <router-link
        v-for="section in sections"
        :key="section.id"
        :to="section.route"
        class="sidebarButton"
        :class="{ active: activeSection === section.id }"
      >
        {{ section.label }}
      </router-link>
    </nav>
  </aside>
</template>

<script setup>
defineProps({
  sections: {
    type: Array,
    required: true,
  },
  activeSection: {
    type: String,
    default: null,
  },
});
</script>

<style scoped>
.adminSidebar {
  width: 200px;
  flex-shrink: 0;
  background-color: var(--bg-elevated-base);
  border-right: 1px solid var(--border-subdued);
  padding: var(--spacing-3);
}

.sidebarNav {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-1);
}

.sidebarButton {
  display: block;
  width: 100%;
  padding: var(--spacing-3) var(--spacing-4);
  text-align: left;
  background: none;
  border: none;
  border-radius: var(--radius-md);
  color: var(--text-subdued);
  font-size: var(--text-base);
  cursor: pointer;
  transition: color var(--transition-fast), background-color var(--transition-fast);
}

.sidebarButton:hover {
  color: var(--text-base);
  background-color: var(--bg-highlight);
}

.sidebarButton.active {
  color: var(--text-base);
  background-color: var(--bg-highlight);
  font-weight: var(--font-medium);
}

/* Responsive: horizontal tabs on mobile */
@media (max-width: 768px) {
  .adminSidebar {
    width: 100%;
    border-right: none;
    border-bottom: 1px solid var(--border-subdued);
    padding: var(--spacing-2);
  }

  .sidebarNav {
    flex-direction: row;
    overflow-x: auto;
    gap: var(--spacing-2);
  }

  .sidebarButton {
    flex-shrink: 0;
    padding: var(--spacing-2) var(--spacing-3);
    font-size: var(--text-sm);
  }
}
</style>
