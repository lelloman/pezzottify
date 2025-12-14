<template>
  <div class="serverControl">
    <h2 class="sectionTitle">Server Control</h2>

    <div class="controlCard">
      <div class="controlInfo">
        <h3 class="controlTitle">Restart Server</h3>
        <p class="controlDescription">
          Initiate a server restart. The server will gracefully shut down and
          restart. All connected clients will be temporarily disconnected.
        </p>
      </div>
      <button
        class="rebootButton"
        :disabled="isRebooting"
        @click="showConfirmDialog = true"
      >
        {{ isRebooting ? "Rebooting..." : "Reboot Server" }}
      </button>
    </div>

    <div v-if="rebootError" class="errorMessage">
      {{ rebootError }}
    </div>

    <h2 class="sectionTitle jobsTitle">Background Jobs</h2>

    <div v-if="jobsLoading" class="loadingMessage">Loading jobs...</div>
    <div v-else-if="jobsError" class="errorMessage">{{ jobsError }}</div>
    <div v-else-if="jobs.length === 0" class="emptyMessage">
      No background jobs registered
    </div>

    <div v-for="job in jobs" :key="job.id" class="jobCard">
      <div class="jobInfo">
        <h3 class="jobTitle">{{ job.name }}</h3>
        <p class="jobDescription">{{ job.description }}</p>
        <div class="jobMeta">
          <span v-if="job.is_running" class="jobStatus running">Running</span>
          <span v-else-if="job.last_run" class="jobStatus">
            Last run: {{ formatLastRun(job.last_run) }}
            <span
              v-if="job.last_run.outcome === 'Success'"
              class="outcome success"
            >
              (Success)
            </span>
            <span v-else class="outcome failed">({{ job.last_run.outcome }})</span>
          </span>
          <span v-else class="jobStatus">Never run</span>
        </div>
      </div>
      <button
        class="triggerButton"
        :disabled="job.is_running || triggeringJobs[job.id]"
        @click="triggerJob(job.id)"
      >
        {{ triggeringJobs[job.id] ? "Triggering..." : job.is_running ? "Running..." : "Run Now" }}
      </button>
    </div>

    <div v-if="triggerError" class="errorMessage triggerError">
      {{ triggerError }}
    </div>

    <ConfirmationDialog
      :isOpen="showConfirmDialog"
      :closeCallback="() => (showConfirmDialog = false)"
      :positiveButtonCallback="handleReboot"
      title="Confirm Server Reboot"
      positiveButtonText="Reboot"
      negativeButtonText="Cancel"
    >
      <template #message>
        Are you sure you want to reboot the server? This will disconnect all
        clients temporarily.
      </template>
    </ConfirmationDialog>
  </div>
</template>

<script setup>
import { ref, watch, onMounted, reactive } from "vue";
import { useRemoteStore } from "@/store/remote";
import ConfirmationDialog from "@/components/common/ConfirmationDialog.vue";
import { wsConnectionStatus } from "@/services/websocket";

const remoteStore = useRemoteStore();

const showConfirmDialog = ref(false);
const isRebooting = ref(false);
const rebootError = ref(null);

// Background jobs state
const jobs = ref([]);
const jobsLoading = ref(true);
const jobsError = ref(null);
const triggeringJobs = reactive({});
const triggerError = ref(null);

// Reset rebooting state when connection is restored after a reboot
watch(wsConnectionStatus, (newStatus, oldStatus) => {
  if (
    isRebooting.value &&
    newStatus === "connected" &&
    oldStatus !== "connected"
  ) {
    isRebooting.value = false;
  }
});

const handleReboot = async () => {
  showConfirmDialog.value = false;
  isRebooting.value = true;
  rebootError.value = null;

  const success = await remoteStore.rebootServer();

  if (!success) {
    rebootError.value = "Failed to initiate server reboot. Please try again.";
    isRebooting.value = false;
  }
  // If successful, the server will restart and we'll lose connection
  // The button stays in "Rebooting..." state
};

const loadJobs = async () => {
  jobsLoading.value = true;
  jobsError.value = null;
  const data = await remoteStore.fetchBackgroundJobs();
  if (data === null) {
    jobsError.value = "Failed to load background jobs";
  } else {
    jobs.value = data;
  }
  jobsLoading.value = false;
};

const triggerJob = async (jobId) => {
  triggeringJobs[jobId] = true;
  triggerError.value = null;

  const result = await remoteStore.triggerBackgroundJob(jobId);

  if (result.error) {
    triggerError.value = `Failed to trigger job: ${result.error}`;
  } else {
    // Refresh job list to show updated status
    await loadJobs();
  }

  triggeringJobs[jobId] = false;
};

const formatLastRun = (lastRun) => {
  if (!lastRun || !lastRun.started_at) return "Unknown";
  const date = new Date(lastRun.started_at);
  return date.toLocaleString();
};

onMounted(() => {
  loadJobs();
});
</script>

<style scoped>
.serverControl {
  max-width: 800px;
}

.sectionTitle {
  font-size: var(--text-2xl);
  font-weight: var(--font-bold);
  color: var(--text-base);
  margin: 0 0 var(--spacing-6) 0;
}

.controlCard {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: var(--spacing-4);
  padding: var(--spacing-4);
  background-color: var(--bg-elevated-base);
  border-radius: var(--radius-lg);
  flex-wrap: wrap;
}

.controlInfo {
  flex: 1;
  min-width: 200px;
}

.controlTitle {
  font-size: var(--text-lg);
  font-weight: var(--font-semibold);
  color: var(--text-base);
  margin: 0 0 var(--spacing-2) 0;
}

.controlDescription {
  font-size: var(--text-sm);
  color: var(--text-subdued);
  margin: 0;
  line-height: 1.5;
}

.rebootButton {
  padding: var(--spacing-3) var(--spacing-6);
  background-color: #dc2626;
  color: white;
  border: none;
  border-radius: var(--radius-md);
  font-size: var(--text-base);
  font-weight: var(--font-medium);
  cursor: pointer;
  transition:
    background-color var(--transition-fast),
    opacity var(--transition-fast);
  flex-shrink: 0;
}

.rebootButton:hover:not(:disabled) {
  background-color: #b91c1c;
}

.rebootButton:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.errorMessage {
  margin-top: var(--spacing-4);
  padding: var(--spacing-3) var(--spacing-4);
  background-color: rgba(220, 38, 38, 0.1);
  border: 1px solid #dc2626;
  border-radius: var(--radius-md);
  color: #dc2626;
  font-size: var(--text-sm);
}

.jobsTitle {
  margin-top: var(--spacing-8);
}

.loadingMessage,
.emptyMessage {
  padding: var(--spacing-4);
  color: var(--text-subdued);
  font-size: var(--text-sm);
}

.jobCard {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: var(--spacing-4);
  padding: var(--spacing-4);
  background-color: var(--bg-elevated-base);
  border-radius: var(--radius-lg);
  flex-wrap: wrap;
  margin-bottom: var(--spacing-3);
}

.jobInfo {
  flex: 1;
  min-width: 200px;
}

.jobTitle {
  font-size: var(--text-lg);
  font-weight: var(--font-semibold);
  color: var(--text-base);
  margin: 0 0 var(--spacing-1) 0;
}

.jobDescription {
  font-size: var(--text-sm);
  color: var(--text-subdued);
  margin: 0 0 var(--spacing-2) 0;
  line-height: 1.4;
}

.jobMeta {
  font-size: var(--text-xs);
  color: var(--text-subdued);
}

.jobStatus.running {
  color: var(--highlight);
  font-weight: var(--font-medium);
}

.outcome.success {
  color: #22c55e;
}

.outcome.failed {
  color: #dc2626;
}

.triggerButton {
  padding: var(--spacing-2) var(--spacing-4);
  background-color: var(--highlight);
  color: var(--text-base);
  border: none;
  border-radius: var(--radius-md);
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  cursor: pointer;
  transition:
    background-color var(--transition-fast),
    opacity var(--transition-fast);
  flex-shrink: 0;
}

.triggerButton:hover:not(:disabled) {
  filter: brightness(1.1);
}

.triggerButton:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.triggerError {
  margin-top: var(--spacing-2);
}
</style>
