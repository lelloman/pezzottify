<template>
  <div class="upload-details">
    <div class="detail-row">
      <span class="label">Filename:</span>
      <span class="value">{{ job?.original_filename || 'Unknown' }}</span>
    </div>

    <div class="detail-row">
      <span class="label">Total Size:</span>
      <span class="value">{{ formatSize(job?.total_size_bytes) }}</span>
    </div>

    <div class="detail-row">
      <span class="label">Files:</span>
      <span class="value">{{ job?.file_count || 0 }}</span>
    </div>

    <div v-if="job?.detected_artist || job?.detected_album" class="detected-metadata">
      <div class="detail-row">
        <span class="label">Detected Artist:</span>
        <span class="value">{{ job?.detected_artist || '-' }}</span>
      </div>
      <div class="detail-row">
        <span class="label">Detected Album:</span>
        <span class="value">{{ job?.detected_album || '-' }}</span>
      </div>
      <div v-if="job?.detected_year" class="detail-row">
        <span class="label">Year:</span>
        <span class="value">{{ job?.detected_year }}</span>
      </div>
    </div>

    <div v-if="files.length > 0" class="files-list">
      <div class="files-header">Files</div>
      <div v-for="file in displayFiles" :key="file.id" class="file-row">
        <span class="file-name" :title="file.filename">{{ truncateFilename(file.filename) }}</span>
        <span class="file-size">{{ formatSize(file.file_size_bytes) }}</span>
        <span v-if="file.duration_ms" class="file-duration">{{ formatDuration(file.duration_ms) }}</span>
      </div>
      <div v-if="files.length > maxDisplayFiles" class="files-more">
        ... and {{ files.length - maxDisplayFiles }} more files
      </div>
    </div>
  </div>
</template>

<script setup>
import { computed } from "vue";

const props = defineProps({
  job: {
    type: Object,
    default: null,
  },
  files: {
    type: Array,
    default: () => [],
  },
});

const maxDisplayFiles = 5;

const displayFiles = computed(() => {
  return props.files.slice(0, maxDisplayFiles);
});

function formatSize(bytes) {
  if (!bytes) return "-";
  if (bytes < 1024) return bytes + " B";
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + " KB";
  return (bytes / (1024 * 1024)).toFixed(1) + " MB";
}

function formatDuration(ms) {
  if (!ms) return "-";
  const seconds = Math.floor(ms / 1000);
  const mins = Math.floor(seconds / 60);
  const secs = seconds % 60;
  return `${mins}:${secs.toString().padStart(2, "0")}`;
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
.upload-details {
  font-size: 13px;
}

.detail-row {
  display: flex;
  margin-bottom: 6px;
}

.label {
  width: 120px;
  color: var(--text-subdued);
  flex-shrink: 0;
}

.value {
  color: var(--text-base);
}

.detected-metadata {
  margin-top: 12px;
  padding-top: 12px;
  border-top: 1px solid var(--border-default);
}

.files-list {
  margin-top: 12px;
  padding-top: 12px;
  border-top: 1px solid var(--border-default);
}

.files-header {
  font-weight: 500;
  margin-bottom: 8px;
  color: var(--text-base);
}

.file-row {
  display: flex;
  gap: 12px;
  padding: 4px 0;
  border-bottom: 1px solid var(--bg-highlight);
}

.file-row:last-child {
  border-bottom: none;
}

.file-name {
  flex: 1;
  color: var(--text-base);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.file-size,
.file-duration {
  color: var(--text-subdued);
  flex-shrink: 0;
}

.files-more {
  padding-top: 8px;
  color: var(--text-subdued);
  font-style: italic;
}
</style>
