<template>
  <div class="settings-container">
    <h1 class="settings-title">Settings</h1>

    <div class="settings-section">
      <p class="no-settings-message">No settings available.</p>
    </div>
  </div>
</template>

<script setup>
// Settings page - placeholder for future settings
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

.no-settings-message {
  color: var(--text-subdued);
  font-style: italic;
  margin: 0;
}
</style>
