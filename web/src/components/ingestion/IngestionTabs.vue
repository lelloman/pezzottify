<template>
  <div class="ingestion-tabs">
    <button
      v-for="session in sessions"
      :key="session.job.id"
      class="tab"
      :class="{ active: session.job.id === activeTabId }"
      @click="$emit('select', session.job.id)"
    >
      <span class="tab-status" :class="statusClass(session.job.status)"></span>
      <span class="tab-name">{{ tabName(session) }}</span>
    </button>
  </div>
</template>

<script setup>
defineProps({
  sessions: {
    type: Array,
    required: true,
  },
  activeTabId: {
    type: String,
    default: null,
  },
});

defineEmits(["select"]);

function statusClass(status) {
  switch (status) {
    case "COMPLETED":
      return "complete";
    case "FAILED":
      return "failed";
    case "AWAITING_REVIEW":
      return "review";
    default:
      return "active";
  }
}

function tabName(session) {
  const job = session.job;
  if (job.detected_album) {
    return job.detected_album;
  }
  if (job.original_filename) {
    const name = job.original_filename;
    if (name.length > 20) {
      return name.substring(0, 17) + "...";
    }
    return name;
  }
  return "Upload";
}
</script>

<style scoped>
.ingestion-tabs {
  display: flex;
  gap: 4px;
  margin-bottom: 12px;
  overflow-x: auto;
  padding-bottom: 4px;
}

.tab {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  border: 1px solid var(--border-default);
  border-radius: 4px;
  background: var(--bg-highlight);
  cursor: pointer;
  font-size: 13px;
  white-space: nowrap;
  color: var(--text-subdued);
}

.tab:hover {
  background: var(--bg-press);
}

.tab.active {
  background: var(--bg-elevated);
  border-color: var(--spotify-green);
  color: var(--text-base);
}

.tab-status {
  width: 8px;
  height: 8px;
  border-radius: 50%;
}

.tab-status.active {
  background: #4a90d9;
  animation: pulse 1.5s infinite;
}

.tab-status.review {
  background: #f5a623;
}

.tab-status.complete {
  background: var(--spotify-green);
}

.tab-status.failed {
  background: #d0021b;
}

@keyframes pulse {
  0%, 100% {
    opacity: 1;
  }
  50% {
    opacity: 0.5;
  }
}

.tab-name {
  color: inherit;
}
</style>
