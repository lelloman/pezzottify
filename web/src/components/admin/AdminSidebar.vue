<template>
  <aside class="adminSidebar" aria-label="Admin sections">
    <nav class="sidebarNav" aria-label="Admin navigation">
      <div v-for="group in groups" :key="group.name" class="navGroup">
        <h2 class="groupTitle">
          {{ group.name }}
        </h2>
        <router-link
          v-for="section in group.sections"
          :key="section.id"
          :to="section.route"
          class="sidebarButton"
          :class="{ active: activeSection === section.id }"
          :aria-current="activeSection === section.id ? 'page' : undefined"
          >{{ section.label }}</router-link
        >
      </div>
    </nav>
    <div class="mobileSectionPicker">
      <label for="admin-section">Section</label>
      <select
        id="admin-section"
        :value="activeSection || ''"
        @change="selectSection"
      >
        <option value="" disabled>Select a section</option>
        <optgroup v-for="group in groups" :key="group.name" :label="group.name">
          <option
            v-for="section in group.sections"
            :key="section.id"
            :value="section.id"
          >
            {{ section.label }}
          </option>
        </optgroup>
      </select>
    </div>
  </aside>
</template>
<script setup>
import { computed } from "vue";
import { useRouter } from "vue-router";
const props = defineProps({
  sections: { type: Array, required: true },
  activeSection: { type: String, default: null },
});
const router = useRouter();
const groups = computed(() =>
  ["Operations", "Catalog", "People", "Insights"]
    .map((name) => ({
      name,
      sections: props.sections.filter((section) => section.group === name),
    }))
    .filter((group) => group.sections.length),
);
const selectSection = (event) => {
  const section = props.sections.find(
    (section) => section.id === event.target.value,
  );
  if (section) router.push(section.route);
};
</script>
<style scoped>
.adminSidebar {
  width: 224px;
  flex-shrink: 0;
  min-height: 0;
  overflow-y: auto;
  background: #121212;
  border-radius: 8px;
  padding: 24px 12px;
}
.sidebarNav {
  display: flex;
  flex-direction: column;
  gap: 20px;
}
.navGroup + .navGroup {
  border-top: 1px solid var(--surface-border);
  padding-top: 20px;
}
.groupTitle {
  margin: 0 12px 10px;
  color: var(--text-subdued);
  font-size: 11px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.1em;
  line-height: 1.5;
}
.sidebarButton {
  display: flex;
  align-items: center;
  min-height: 44px;
  padding: 10px 12px;
  border-radius: 4px;
  color: var(--text-subdued);
  text-decoration: none;
  font-size: 14px;
  font-weight: 500;
}
.sidebarButton:hover {
  color: #fff;
  background: #1f1f1f;
}
.sidebarButton.active {
  background: #2a2a2a;
  color: #fff;
  font-weight: 700;
}
.sidebarButton:focus-visible,
select:focus-visible {
  outline: 2px solid #fff;
  outline-offset: -2px;
}
.mobileSectionPicker {
  display: none;
}
@media (max-width: 767px) {
  .adminSidebar {
    width: 100%;
    padding: 12px 16px;
    overflow: visible;
  }
  .sidebarNav {
    display: none;
  }
  .mobileSectionPicker {
    display: flex;
    align-items: center;
    gap: 16px;
  }
  label {
    color: var(--text-subdued);
    font-size: 13px;
  }
  select {
    flex: 1;
    min-width: 0;
    min-height: 40px;
    padding: 8px 12px;
    background: #242424;
    color: var(--text-base);
    color-scheme: dark;
    border: 1px solid #727272;
    border-radius: 4px;
    font: inherit;
    font-size: 14px;
  }
}
</style>
