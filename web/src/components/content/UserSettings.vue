<template>
  <div class="settings-container">
    <h1 class="settings-title">Settings</h1>

    <div class="settings-section">
      <h2 class="section-title">Search</h2>
      <div class="setting-row">
        <div class="setting-info">
          <span class="setting-label">Organic Search</span>
          <span class="setting-description">
            Use classic flat search results. When disabled, uses smart search
            with intelligent result grouping and enrichment.
          </span>
        </div>
        <label class="toggle">
          <input type="checkbox" v-model="useOrganicSearch" />
          <span class="toggle-slider"></span>
        </label>
      </div>
    </div>

    <div class="settings-section">
      <h2 class="section-title">Display</h2>
      <div class="setting-row">
        <div class="setting-info">
          <span class="setting-label">Show Images</span>
          <span class="setting-description">
            Display album and artist images throughout the app.
          </span>
        </div>
        <label class="toggle">
          <input type="checkbox" v-model="imagesEnabled" />
          <span class="toggle-slider"></span>
        </label>
      </div>
    </div>
  </div>
</template>

<script setup>
import { useDebugStore } from "@/store/debug";
import { storeToRefs } from "pinia";

const debugStore = useDebugStore();
const { useOrganicSearch, imagesEnabled } = storeToRefs(debugStore);
</script>

<style scoped>
.settings-container {
  max-width: 800px;
  margin: 0 auto;
  padding: var(--spacing-4);
}

.settings-title {
  font-size: var(--text-2xl);
  font-weight: var(--font-bold);
  color: var(--text-base);
  margin-bottom: var(--spacing-6);
}

.settings-section {
  background-color: var(--bg-elevated-base);
  border-radius: var(--radius-lg);
  padding: var(--spacing-4);
  margin-bottom: var(--spacing-4);
}

.section-title {
  font-size: var(--text-lg);
  font-weight: var(--font-semibold);
  color: var(--text-base);
  margin: 0 0 var(--spacing-4) 0;
}

.setting-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--spacing-4);
  padding: var(--spacing-3) 0;
  border-bottom: 1px solid var(--bg-elevated-highlight);
}

.setting-row:last-child {
  border-bottom: none;
}

.setting-info {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-1);
  flex: 1;
}

.setting-label {
  font-weight: var(--font-medium);
  color: var(--text-base);
}

.setting-description {
  font-size: var(--text-sm);
  color: var(--text-subdued);
}

/* Toggle switch */
.toggle {
  position: relative;
  display: inline-block;
  width: 48px;
  height: 28px;
  flex-shrink: 0;
}

.toggle input {
  opacity: 0;
  width: 0;
  height: 0;
}

.toggle-slider {
  position: absolute;
  cursor: pointer;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: var(--bg-elevated-highlight);
  transition: 0.3s;
  border-radius: 28px;
}

.toggle-slider:before {
  position: absolute;
  content: "";
  height: 22px;
  width: 22px;
  left: 3px;
  bottom: 3px;
  background-color: white;
  transition: 0.3s;
  border-radius: 50%;
}

.toggle input:checked + .toggle-slider {
  background-color: var(--accent-color);
}

.toggle input:checked + .toggle-slider:before {
  transform: translateX(20px);
}
</style>
