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

    <h2 class="sectionTitle auditTitle">Job Audit Log</h2>

    <div v-if="auditLoading" class="loadingMessage">Loading audit log...</div>
    <div v-else-if="auditError" class="errorMessage">{{ auditError }}</div>
    <div v-else-if="auditEntries.length === 0" class="emptyMessage">
      No audit log entries
    </div>

    <div v-else class="auditTable">
      <div class="auditHeader">
        <span class="auditCol time">Time</span>
        <span class="auditCol job">Job</span>
        <span class="auditCol event">Event</span>
        <span class="auditCol duration">Duration</span>
        <span class="auditCol details">Details</span>
      </div>
      <div v-for="entry in auditEntries" :key="entry.id" class="auditRow">
        <span class="auditCol time">{{ formatTimestamp(entry.timestamp) }}</span>
        <span class="auditCol job">{{ entry.job_id }}</span>
        <span class="auditCol event">
          <span :class="['eventBadge', entry.event_type]">{{ entry.event_type }}</span>
        </span>
        <span class="auditCol duration">{{ formatDuration(entry.duration_ms) }}</span>
        <span class="auditCol details">
          <span v-if="entry.error" class="errorText">{{ entry.error }}</span>
          <span v-else-if="entry.details" class="detailsText">{{ formatDetails(entry.details) }}</span>
          <span v-else class="noDetails">-</span>
        </span>
      </div>
    </div>

    <button v-if="auditEntries.length > 0" class="refreshButton" @click="loadAuditLog">
      Refresh
    </button>

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

// Audit log state
const auditEntries = ref([]);
const auditLoading = ref(true);
const auditError = ref(null);

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

const loadAuditLog = async () => {
  auditLoading.value = true;
  auditError.value = null;
  const data = await remoteStore.fetchJobAuditLog(50, 0);
  if (data === null) {
    auditError.value = "Failed to load audit log";
  } else {
    auditEntries.value = data;
  }
  auditLoading.value = false;
};

const formatTimestamp = (timestamp) => {
  const date = new Date(timestamp * 1000);
  return date.toLocaleString();
};

const formatDuration = (durationMs) => {
  if (durationMs === null || durationMs === undefined) return "-";
  if (durationMs < 1000) return `${durationMs}ms`;
  const seconds = (durationMs / 1000).toFixed(1);
  return `${seconds}s`;
};

const formatDetails = (details) => {
  if (!details) return "-";

  const parts = [];

  // IntegrityWatchdog - completed
  if (details.is_clean !== undefined) {
    if (details.is_clean) {
      parts.push("✓ Catalog clean");
    } else {
      if (details.total_missing > 0) {
        const missing = [];
        if (details.missing_track_audio_count > 0) missing.push(`${details.missing_track_audio_count} tracks`);
        if (details.missing_album_images_count > 0) missing.push(`${details.missing_album_images_count} album images`);
        if (details.missing_artist_images_count > 0) missing.push(`${details.missing_artist_images_count} artist images`);
        parts.push(`Missing: ${missing.join(", ")}`);
      }
      if (details.total_artist_enrichment > 0) {
        const enrichment = [];
        if (details.artists_without_related_count > 0) enrichment.push(`${details.artists_without_related_count} without related`);
        if (details.orphan_related_artist_ids_count > 0) enrichment.push(`${details.orphan_related_artist_ids_count} orphan relations`);
        parts.push(`Artist enrichment: ${enrichment.join(", ")}`);
      }
    }
    if (details.items_queued > 0) parts.push(`Queued: ${details.items_queued}`);
    if (details.items_skipped > 0) parts.push(`Skipped: ${details.items_skipped}`);
  }

  // PopularContent - started
  if (details.start_date !== undefined && details.end_date !== undefined) {
    parts.push(`Date range: ${details.start_date} - ${details.end_date}`);
    if (details.lookback_days) parts.push(`${details.lookback_days} day lookback`);
  }

  // PopularContent - completed
  if (details.albums_count !== undefined && details.artists_count !== undefined) {
    parts.push(`${details.albums_count} albums, ${details.artists_count} artists`);
    if (details.tracks_analyzed) parts.push(`${details.tracks_analyzed} tracks analyzed`);
  }

  // PopularContent - skipped
  if (details.skipped) {
    parts.push(`Skipped: ${details.reason === "no_listening_data" ? "No listening data" : details.reason}`);
  }

  // AuditLogCleanup - started
  if (details.retention_days !== undefined && parts.length === 0) {
    parts.push(`Retention: ${details.retention_days} days`);
  }

  // AuditLogCleanup - completed
  if (details.total_deleted !== undefined) {
    if (details.total_deleted === 0) {
      parts.push("No entries to clean up");
    } else {
      const deleted = [];
      if (details.download_entries_deleted > 0) deleted.push(`${details.download_entries_deleted} download`);
      if (details.job_entries_deleted > 0) deleted.push(`${details.job_entries_deleted} job`);
      parts.push(`Deleted: ${deleted.join(", ")} entries`);
    }
  }

  return parts.length > 0 ? parts.join(" • ") : "-";
};

onMounted(() => {
  loadJobs();
  loadAuditLog();
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

.auditTitle {
  margin-top: var(--spacing-8);
}

.auditTable {
  background-color: var(--bg-elevated-base);
  border-radius: var(--radius-lg);
  overflow: hidden;
}

.auditHeader,
.auditRow {
  display: grid;
  grid-template-columns: 160px 140px 100px 80px 1fr;
  padding: var(--spacing-3) var(--spacing-4);
  gap: var(--spacing-2);
}

.auditHeader {
  background-color: var(--bg-elevated-highlight);
  font-weight: var(--font-semibold);
  font-size: var(--text-sm);
  color: var(--text-subdued);
  border-bottom: 1px solid var(--border-subtle);
}

.auditRow {
  font-size: var(--text-sm);
  border-bottom: 1px solid var(--border-subtle);
}

.auditRow:last-child {
  border-bottom: none;
}

.auditCol {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.auditCol.details {
  white-space: normal;
  word-break: break-word;
}

.eventBadge {
  display: inline-block;
  padding: var(--spacing-1) var(--spacing-2);
  border-radius: var(--radius-sm);
  font-size: var(--text-xs);
  font-weight: var(--font-medium);
  text-transform: uppercase;
}

.eventBadge.started {
  background-color: rgba(59, 130, 246, 0.2);
  color: #3b82f6;
}

.eventBadge.completed {
  background-color: rgba(34, 197, 94, 0.2);
  color: #22c55e;
}

.eventBadge.failed {
  background-color: rgba(220, 38, 38, 0.2);
  color: #dc2626;
}

.eventBadge.progress {
  background-color: rgba(168, 85, 247, 0.2);
  color: #a855f7;
}

.errorText {
  color: #dc2626;
}

.detailsText {
  color: var(--text-subdued);
}

.noDetails {
  color: var(--text-subdued);
  opacity: 0.5;
}

.refreshButton {
  margin-top: var(--spacing-4);
  padding: var(--spacing-2) var(--spacing-4);
  background-color: var(--bg-elevated-highlight);
  color: var(--text-base);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  font-size: var(--text-sm);
  cursor: pointer;
  transition: background-color var(--transition-fast);
}

.refreshButton:hover {
  background-color: var(--bg-elevated-base);
}
</style>
