<template>
  <div class="status-bar">
    <div class="status-phases">
      <div
        v-for="phase in phases"
        :key="phase.id"
        class="phase"
        :class="{ active: isActive(phase), complete: isComplete(phase) }"
      >
        <div class="phase-indicator">
          <span v-if="isComplete(phase)" class="check">&#10003;</span>
          <span v-else-if="isActive(phase)" class="spinner"></span>
          <span v-else class="dot"></span>
        </div>
        <span class="phase-label">{{ phase.label }}</span>
      </div>
    </div>

    <div v-if="showProgress" class="progress-bar">
      <div class="progress-fill" :style="{ width: progressPercent + '%' }"></div>
    </div>

    <div v-if="statusMessage" class="status-message">
      {{ statusMessage }}
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
  session: {
    type: Object,
    default: null,
  },
});

const phases = [
  { id: "analyzing", label: "Analyze", statuses: ["PENDING", "ANALYZING"] },
  { id: "identifying", label: "Identify", statuses: ["IDENTIFYING_ALBUM", "AWAITING_REVIEW"] },
  { id: "mapping", label: "Map", statuses: ["MAPPING_TRACKS"] },
  { id: "converting", label: "Convert", statuses: ["CONVERTING"] },
  { id: "complete", label: "Done", statuses: ["COMPLETED", "FAILED"] },
];

const currentPhaseIndex = computed(() => {
  const status = props.job?.status;
  if (!status) return 0;
  return phases.findIndex((p) => p.statuses.includes(status));
});

function isActive(phase) {
  const idx = phases.indexOf(phase);
  return idx === currentPhaseIndex.value && props.job?.status !== "COMPLETED" && props.job?.status !== "FAILED";
}

function isComplete(phase) {
  const idx = phases.indexOf(phase);
  if (idx < currentPhaseIndex.value) return true;
  if (idx === currentPhaseIndex.value && props.job?.status === "COMPLETED") return true;
  return false;
}

const showProgress = computed(() => {
  return props.session?.phaseProgress > 0 && props.session?.phaseProgress < 100;
});

const progressPercent = computed(() => {
  return props.session?.phaseProgress || 0;
});

const statusMessage = computed(() => {
  const status = props.job?.status;
  const session = props.session;

  if (status === "FAILED") {
    return props.job?.error_message || "Failed";
  }

  if (status === "COMPLETED") {
    const converted = props.job?.tracks_converted || 0;
    return `Successfully added ${converted} tracks`;
  }

  if (status === "AWAITING_REVIEW") {
    return "Review required";
  }

  if (session?.phase === "analyzing") {
    return `Analyzing ${session.filesProcessed || 0}/${session.filesTotal || 0} files...`;
  }

  if (session?.phase === "converting") {
    return `Converting ${session.filesProcessed || 0}/${session.filesTotal || 0} files...`;
  }

  return null;
});
</script>

<style scoped>
.status-bar {
  padding: 12px 0;
  border-bottom: 1px solid var(--border-default);
  margin-bottom: 12px;
}

.status-phases {
  display: flex;
  justify-content: space-between;
  margin-bottom: 8px;
}

.phase {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  opacity: 0.4;
}

.phase.active,
.phase.complete {
  opacity: 1;
}

.phase-indicator {
  width: 24px;
  height: 24px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 50%;
  background: var(--bg-highlight);
  font-size: 12px;
  color: var(--text-subdued);
}

.phase.complete .phase-indicator {
  background: var(--spotify-green);
  color: var(--text-negative);
}

.phase.active .phase-indicator {
  background: #4a90d9;
  color: white;
}

.check {
  font-weight: bold;
}

.dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--text-subtle);
}

.spinner {
  width: 12px;
  height: 12px;
  border: 2px solid transparent;
  border-top-color: white;
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.phase-label {
  font-size: 11px;
  color: var(--text-subtle);
}

.phase.active .phase-label,
.phase.complete .phase-label {
  color: var(--text-base);
  font-weight: 500;
}

.progress-bar {
  height: 4px;
  background: var(--bg-highlight);
  border-radius: 2px;
  overflow: hidden;
  margin: 8px 0;
}

.progress-fill {
  height: 100%;
  background: #4a90d9;
  transition: width 0.3s ease;
}

.status-message {
  font-size: 13px;
  color: var(--text-subdued);
  text-align: center;
}
</style>
