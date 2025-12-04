<template>
  <div class="settings-container">
    <h1 class="settings-title">Settings</h1>

    <div class="settings-section">
      <h2 class="section-title">Content Downloads</h2>
      <div class="setting-item">
        <div class="setting-info">
          <label class="setting-label" for="direct-downloads">Enable Direct Downloads</label>
          <p class="setting-description">
            When enabled, missing content (albums, artists) will be automatically fetched from the server
            when you browse to them. This requires the appropriate permission from an administrator.
          </p>
        </div>
        <label class="toggle">
          <input
            type="checkbox"
            id="direct-downloads"
            :checked="isDirectDownloadsEnabled"
            @change="handleDirectDownloadsToggle"
            :disabled="isUpdating"
          />
          <span class="toggle-slider"></span>
        </label>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed } from 'vue';
import { useUserStore } from '@/store/user';

const userStore = useUserStore();
const isUpdating = ref(false);

const isDirectDownloadsEnabled = computed(() => userStore.isDirectDownloadsEnabled);

const handleDirectDownloadsToggle = async (event) => {
  const newValue = event.target.checked;
  isUpdating.value = true;
  try {
    const success = await userStore.setDirectDownloadsEnabled(newValue);
    if (!success) {
      // Revert the checkbox if the update failed
      event.target.checked = !newValue;
    }
  } finally {
    isUpdating.value = false;
  }
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
  height: 24px;
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
  background-color: var(--bg-subdued);
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
  background-color: var(--text-base);
  transition: 0.3s;
  border-radius: 50%;
}

.toggle input:checked + .toggle-slider {
  background-color: var(--spotify-green);
}

.toggle input:checked + .toggle-slider:before {
  transform: translateX(24px);
}

.toggle input:disabled + .toggle-slider {
  opacity: 0.5;
  cursor: not-allowed;
}
</style>
