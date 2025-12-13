<template>
  <div class="settings-container">
    <h1 class="settings-title">Settings</h1>

    <div class="settings-section">
      <h2 class="section-title">Content Downloads</h2>
      <div class="setting-item">
        <div class="setting-info">
          <label class="setting-label" for="direct-downloads">
            Enable Direct Downloads
            <span
              v-if="isDirectDownloadsPending"
              class="sync-pending"
              title="Syncing..."
            >
              <span class="sync-dot"></span>
            </span>
          </label>
          <p class="setting-description">
            When enabled, missing content (albums, artists) will be
            automatically fetched from the server when you browse to them. This
            requires the appropriate permission from an administrator.
          </p>
        </div>
        <label class="toggle">
          <input
            type="checkbox"
            id="direct-downloads"
            :checked="isDirectDownloadsEnabled"
            @change="handleDirectDownloadsToggle"
          />
          <span class="toggle-slider"></span>
        </label>
      </div>
    </div>

    <div v-if="canRequestContent" class="settings-section">
      <h2 class="section-title">External Search</h2>
      <div class="setting-item">
        <div class="setting-info">
          <label class="setting-label" for="external-search">
            Enable External Search
            <span
              v-if="isExternalSearchPending"
              class="sync-pending"
              title="Syncing..."
            >
              <span class="sync-dot"></span>
            </span>
          </label>
          <p class="setting-description">
            When enabled, searches will also query external providers for
            content that can be requested for download. Results from external
            sources will appear in a separate section.
          </p>
        </div>
        <label class="toggle">
          <input
            type="checkbox"
            id="external-search"
            :checked="isExternalSearchEnabled"
            @change="handleExternalSearchToggle"
          />
          <span class="toggle-slider"></span>
        </label>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed } from "vue";
import { useUserStore } from "@/store/user";

const userStore = useUserStore();

// Direct downloads setting
const isDirectDownloadsEnabled = computed(
  () => userStore.isDirectDownloadsEnabled,
);
const isDirectDownloadsPending = computed(
  () => userStore.isDirectDownloadsPending,
);

const handleDirectDownloadsToggle = async (event) => {
  const newValue = event.target.checked;
  await userStore.setDirectDownloadsEnabled(newValue);
};

// External search setting (only visible with RequestContent permission)
const canRequestContent = computed(() => userStore.canRequestContent);

const isExternalSearchEnabled = computed(
  () => userStore.isExternalSearchEnabled,
);
const isExternalSearchPending = computed(
  () => userStore.isExternalSearchPending,
);

const handleExternalSearchToggle = async (event) => {
  const newValue = event.target.checked;
  await userStore.setExternalSearchEnabled(newValue);
};
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
  margin-bottom: var(--spacing-4);
  padding-bottom: var(--spacing-2);
  border-bottom: 1px solid var(--border-subdued);
}

.setting-item {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: var(--spacing-4);
  padding: var(--spacing-3) 0;
  flex-wrap: wrap;
}

.setting-info {
  flex: 1;
}

.setting-label {
  font-size: var(--text-base);
  font-weight: var(--font-medium);
  color: var(--text-base);
  display: block;
  margin-bottom: var(--spacing-1);
}

.setting-description {
  font-size: var(--text-sm);
  color: var(--text-subdued);
  line-height: 1.5;
  margin: 0;
}

/* Toggle switch styles */
.toggle {
  position: relative;
  display: inline-block;
  width: 48px;
  min-width: 48px;
  height: 24px;
  flex-shrink: 0;
  margin-left: auto;
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
  background-color: var(--bg-subdued, #535353);
  transition: 0.3s;
  border-radius: 24px;
}

.toggle-slider:before {
  position: absolute;
  content: "";
  height: 18px;
  width: 18px;
  left: 3px;
  bottom: 3px;
  background-color: var(--text-base, #fff);
  transition: 0.3s;
  border-radius: 50%;
}

.toggle input:checked + .toggle-slider {
  background-color: var(--spotify-green, #1db954);
}

.toggle input:checked + .toggle-slider:before {
  transform: translateX(24px);
}

.toggle input:disabled + .toggle-slider {
  opacity: 0.5;
  cursor: not-allowed;
}

/* Sync pending indicator */
.sync-pending {
  display: inline-flex;
  align-items: center;
  margin-left: var(--spacing-2);
}

.sync-dot {
  width: 8px;
  height: 8px;
  background-color: var(--spotify-green);
  border-radius: 50%;
  animation: pulse 1.5s ease-in-out infinite;
}

@keyframes pulse {
  0%,
  100% {
    opacity: 1;
    transform: scale(1);
  }
  50% {
    opacity: 0.5;
    transform: scale(0.8);
  }
}
</style>
