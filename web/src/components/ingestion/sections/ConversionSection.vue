<template>
  <div class="conversion-section">
    <div class="conversion-summary">
      <div class="summary-item">
        <span class="summary-value">{{ convertedCount }}</span>
        <span class="summary-label">Converted</span>
      </div>
      <div class="summary-item">
        <span class="summary-value">{{ pendingCount }}</span>
        <span class="summary-label">Pending</span>
      </div>
      <div v-if="skippedCount > 0" class="summary-item">
        <span class="summary-value">{{ skippedCount }}</span>
        <span class="summary-label">Skipped</span>
      </div>
      <div v-if="failedCount > 0" class="summary-item error">
        <span class="summary-value">{{ failedCount }}</span>
        <span class="summary-label">Failed</span>
      </div>
    </div>

    <div class="files-list">
      <div
        v-for="file in files"
        :key="file.id"
        class="file-row"
        :class="statusClass(file)"
      >
        <span class="file-icon">{{ statusIcon(file) }}</span>
        <span class="file-name" :title="file.filename">{{ truncateFilename(file.filename) }}</span>
        <span class="file-status">{{ statusLabel(file) }}</span>
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed } from "vue";

const props = defineProps({
  files: {
    type: Array,
    default: () => [],
  },
  job: {
    type: Object,
    default: null,
  },
});

const convertedCount = computed(() => {
  return props.files.filter((f) => f.converted).length;
});

const pendingCount = computed(() => {
  return props.files.filter((f) => !f.converted && !f.error_message && !isSkipped(f)).length;
});

const skippedCount = computed(() => {
  return props.files.filter((f) => isSkipped(f)).length;
});

const failedCount = computed(() => {
  return props.files.filter((f) => f.error_message).length;
});

function isSkipped(file) {
  const reason = file.conversion_reason;
  return reason && reason.type === "NoConversionNeeded";
}

function statusClass(file) {
  if (file.error_message) return "error";
  if (file.converted) return "success";
  if (isSkipped(file)) return "skipped";
  return "pending";
}

function statusIcon(file) {
  if (file.error_message) return "!";
  if (file.converted) return "\u2713";
  if (isSkipped(file)) return "-";
  return "\u2022";
}

function statusLabel(file) {
  if (file.error_message) return "Failed";
  if (file.converted) return "Done";
  if (isSkipped(file)) return "Skipped";
  return "Pending";
}

function truncateFilename(name) {
  if (!name) return "-";
  if (name.length > 40) {
    return name.substring(0, 37) + "...";
  }
  return name;
}
</script>

<style scoped>
.conversion-section {
  font-size: 13px;
}

.conversion-summary {
  display: flex;
  gap: 24px;
  margin-bottom: 16px;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--border-default);
}

.summary-item {
  text-align: center;
}

.summary-value {
  display: block;
  font-size: 24px;
  font-weight: 600;
  color: var(--text-base);
}

.summary-item.error .summary-value {
  color: #d0021b;
}

.summary-label {
  font-size: 12px;
  color: var(--text-subdued);
}

.files-list {
  max-height: 200px;
  overflow-y: auto;
}

.file-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 0;
  border-bottom: 1px solid var(--bg-highlight);
}

.file-row:last-child {
  border-bottom: none;
}

.file-icon {
  width: 16px;
  text-align: center;
  font-weight: bold;
}

.file-row.success .file-icon {
  color: var(--spotify-green);
}

.file-row.error .file-icon {
  color: #d0021b;
}

.file-row.skipped .file-icon {
  color: var(--text-subtle);
}

.file-row.pending .file-icon {
  color: #4a90d9;
}

.file-name {
  flex: 1;
  color: var(--text-base);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.file-status {
  font-size: 12px;
  color: var(--text-subdued);
}

.file-row.success .file-status {
  color: var(--spotify-green);
}

.file-row.error .file-status {
  color: #d0021b;
}
</style>
