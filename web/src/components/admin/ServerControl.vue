<template>
  <div class="serverControl">
    <header class="pageHeader">
      <h2 class="pageTitle">Server</h2>
      <p>Monitor storage, tune search and manage background jobs.</p>
    </header>

    <div class="controlCard">
      <div class="controlInfo">
        <h3 class="controlTitle">Restart server</h3>
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

    <h2 class="sectionTitle storageTitle">Storage</h2>

    <div class="storageCard">
      <div class="storageHeader">
        <div>
          <h3 class="controlTitle">Disk usage</h3>
          <p class="controlDescription">
            Storage used by SQLite databases, media, and operational upload
            data.
          </p>
        </div>
        <button
          class="refreshButton small"
          :disabled="storageLoading"
          @click="loadStorageReport"
        >
          {{ storageLoading ? "Refreshing..." : "Refresh" }}
        </button>
      </div>

      <div v-if="storageLoading" class="loadingMessage">
        Loading storage usage...
      </div>
      <div v-else-if="storageError" class="errorMessage">
        {{ storageError }}
      </div>
      <div v-else-if="storageReport" class="storageBody">
        <div class="storageStats">
          <div class="storageStat">
            <span class="statValue">{{
              formatBytes(storageReport.total_bytes)
            }}</span>
            <span class="statLabel">Total tracked</span>
          </div>
          <div class="storageStat">
            <span class="statValue">{{
              formatBytes(storageReport.database_total_bytes)
            }}</span>
            <span class="statLabel">Databases</span>
          </div>
          <div class="storageStat">
            <span class="statValue">{{
              formatBytes(storageReport.filesystem_total_bytes)
            }}</span>
            <span class="statLabel">Media and uploads</span>
          </div>
        </div>

        <div class="storageRows">
          <div
            v-for="db in storageReport.databases"
            :key="`db-${db.id}`"
            class="storageRow"
          >
            <div class="storageMeta">
              <span class="storageName">{{ db.label }}</span>
              <span class="storagePath" :title="db.path">{{ db.path }}</span>
              <span
                v-if="db.wal_bytes || db.shm_bytes"
                class="storageBreakdown"
              >
                DB {{ formatBytes(db.main_bytes) }} · WAL
                {{ formatBytes(db.wal_bytes) }} · SHM
                {{ formatBytes(db.shm_bytes) }}
              </span>
            </div>
            <div class="storageMeasure">
              <div class="storageBar">
                <span :style="{ width: storagePercent(db.total_bytes) }"></span>
              </div>
              <span class="storageSize">{{ formatBytes(db.total_bytes) }}</span>
            </div>
          </div>

          <div
            v-for="component in storageReport.components"
            :key="`component-${component.id}`"
            class="storageRow"
          >
            <div class="storageMeta">
              <span class="storageName">{{ component.label }}</span>
              <span class="storagePath" :title="component.path">{{
                component.path
              }}</span>
            </div>
            <div class="storageMeasure">
              <div class="storageBar">
                <span
                  :style="{ width: storagePercent(component.bytes) }"
                ></span>
              </div>
              <span class="storageSize">{{
                formatBytes(component.bytes)
              }}</span>
            </div>
          </div>
        </div>
      </div>
    </div>

    <h2 class="sectionTitle searchTitle">Search settings</h2>

    <div class="controlCard searchSettings">
      <div class="controlInfo">
        <h3 class="controlTitle">Relevance filter</h3>
        <p class="controlDescription">
          Filter search results to remove low-quality matches. Choose a
          filtering method and configure its parameters.
        </p>
      </div>

      <div v-if="filterLoading" class="loadingMessage">Loading...</div>
      <div v-else-if="filterError" class="errorMessage">{{ filterError }}</div>
      <div v-else class="filterConfig">
        <div class="filterRow">
          <label class="filterLabel" for="server-filter-1">Method</label>
          <select
            id="server-filter-1"
            v-model="filterMethod"
            class="filterSelect"
            @change="onFilterMethodChange"
          >
            <option value="none">None (return all results)</option>
            <option value="percentage_of_best">Percentage of Best</option>
            <option value="gap_detection">Gap Detection</option>
            <option value="standard_deviation">Standard Deviation</option>
            <option value="percentage_with_minimum">
              Percentage with Minimum
            </option>
          </select>
        </div>

        <div v-if="filterMethod === 'percentage_of_best'" class="filterRow">
          <label class="filterLabel" for="server-filter-2"
            >Threshold (0-1)</label
          >
          <input
            id="server-filter-2"
            v-model.number="filterParams.threshold"
            type="number"
            step="0.1"
            min="0"
            max="1"
            class="filterInput"
          />
          <span class="filterHint"
            >Keep results with score >=
            {{ (filterParams.threshold * 100).toFixed(0) }}% of best</span
          >
        </div>

        <div v-if="filterMethod === 'gap_detection'" class="filterRow">
          <label class="filterLabel" for="server-filter-3"
            >Drop Threshold (0-1)</label
          >
          <input
            id="server-filter-3"
            v-model.number="filterParams.drop_threshold"
            type="number"
            step="0.1"
            min="0"
            max="1"
            class="filterInput"
          />
          <span class="filterHint"
            >Cut when next score drops below
            {{ (filterParams.drop_threshold * 100).toFixed(0) }}% of
            previous</span
          >
        </div>

        <div v-if="filterMethod === 'standard_deviation'" class="filterRow">
          <label class="filterLabel" for="server-filter-4"
            >Std Deviations</label
          >
          <input
            id="server-filter-4"
            v-model.number="filterParams.num_std_devs"
            type="number"
            step="0.5"
            min="0"
            class="filterInput"
          />
          <span class="filterHint"
            >Keep results within {{ filterParams.num_std_devs }} std devs of
            mean</span
          >
        </div>

        <div
          v-if="filterMethod === 'percentage_with_minimum'"
          class="filterRow"
        >
          <label class="filterLabel" for="server-filter-5"
            >Threshold (0-1)</label
          >
          <input
            id="server-filter-5"
            v-model.number="filterParams.threshold"
            type="number"
            step="0.1"
            min="0"
            max="1"
            class="filterInput"
          />
        </div>
        <div
          v-if="filterMethod === 'percentage_with_minimum'"
          class="filterRow"
        >
          <label class="filterLabel" for="server-filter-6"
            >Min Best Score</label
          >
          <input
            id="server-filter-6"
            v-model.number="filterParams.min_best_score"
            type="number"
            step="1000"
            min="0"
            class="filterInput"
          />
          <span class="filterHint"
            >Only filter if best score exceeds this value</span
          >
        </div>

        <div class="filterActions">
          <button
            class="saveButton"
            :disabled="filterSaving"
            @click="saveFilter"
          >
            {{ filterSaving ? "Saving..." : "Save" }}
          </button>
          <span v-if="filterSaveSuccess" class="saveSuccess">Saved!</span>
          <span v-if="filterSaveError" class="saveError">{{
            filterSaveError
          }}</span>
        </div>
      </div>
    </div>

    <h2 class="sectionTitle jobsTitle">Background jobs</h2>

    <div class="embeddingCoverageCard">
      <div class="coverageHeader">
        <div>
          <h3 class="controlTitle">Audio embedding coverage</h3>
          <p class="controlDescription">
            Available tracks with MusicFM/AST embeddings stored in the catalog.
          </p>
        </div>
        <button
          class="refreshButton small"
          :disabled="embeddingCoverageLoading"
          @click="loadEmbeddingCoverage"
        >
          {{ embeddingCoverageLoading ? "Refreshing..." : "Refresh" }}
        </button>
      </div>

      <div v-if="embeddingCoverageLoading" class="loadingMessage">
        Loading embedding coverage...
      </div>
      <div v-else-if="embeddingCoverageError" class="errorMessage">
        {{ embeddingCoverageError }}
      </div>
      <div v-else-if="embeddingCoverage" class="coverageBody">
        <div class="coverageStats">
          <div class="coverageStat">
            <span class="statValue">{{
              formatNumber(embeddingCoverage.coverage.available_tracks)
            }}</span>
            <span class="statLabel">Available tracks</span>
          </div>
          <div class="coverageStat">
            <span class="statValue">{{
              formatNumber(embeddingCoverage.coverage.fully_embedded_tracks)
            }}</span>
            <span class="statLabel">Complete</span>
          </div>
          <div class="coverageStat warning">
            <span class="statValue">{{
              formatNumber(
                embeddingCoverage.coverage.tracks_missing_any_embedding,
              )
            }}</span>
            <span class="statLabel">Missing any embedding</span>
          </div>
        </div>

        <div class="coverageNamespaces">
          <div
            v-for="namespace in embeddingCoverage.coverage.namespaces"
            :key="namespace.namespace"
            class="namespaceRow"
          >
            <div class="namespaceInfo">
              <span class="namespaceName">{{ namespace.namespace }}</span>
              <span class="namespaceModel">{{
                modelForNamespace(namespace.namespace)
              }}</span>
            </div>
            <div class="namespaceCounts">
              <span>{{ formatNumber(namespace.embedded_tracks) }} present</span>
              <span>{{ formatNumber(namespace.missing_tracks) }} missing</span>
            </div>
          </div>
        </div>

        <p v-if="!embeddingCoverage.enabled" class="coverageWarning">
          Embedding sync is not enabled in server config; these counts use the
          default namespaces.
        </p>
      </div>
    </div>

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
            <span v-else class="outcome failed"
              >({{ job.last_run.outcome }})</span
            >
          </span>
          <span v-else class="jobStatus">Never run</span>
        </div>
      </div>
      <div v-if="job.id === 'expand_artists_base'" class="jobOptions">
        <label class="modeToggle">
          <input
            type="checkbox"
            :checked="expandArtistsMode === 'actual'"
            @change="
              expandArtistsMode = $event.target.checked ? 'actual' : 'dry_run'
            "
          />
          <span class="modeLabel">{{
            expandArtistsMode === "actual"
              ? "Actual (will queue downloads)"
              : "Dry-run (preview only)"
          }}</span>
        </label>
      </div>
      <div v-if="job.id === 'missing_files_watchdog'" class="jobOptions">
        <label class="modeToggle">
          <input
            type="checkbox"
            :checked="missingFilesMode === 'actual'"
            @change="
              missingFilesMode = $event.target.checked ? 'actual' : 'dry_run'
            "
          />
          <span class="modeLabel">{{
            missingFilesMode === "actual"
              ? "Actual (will queue downloads)"
              : "Dry-run (preview only)"
          }}</span>
        </label>
      </div>
      <div
        v-if="job.id === 'track_embedding_sync'"
        class="jobOptions embeddingOptions"
      >
        <label class="numberOption">
          <span>Max tracks</span>
          <input
            v-model.number="embeddingMaxTracks"
            type="number"
            min="1"
            step="100"
          />
        </label>
        <label class="modeToggle">
          <input v-model="embeddingForce" type="checkbox" />
          <span class="modeLabel">Force regenerate</span>
        </label>
      </div>
      <div
        v-if="job.id === 'metadata_enrichment_v1'"
        class="jobOptions metadataOptions"
      >
        <label class="numberOption">
          <span>Max items</span>
          <input
            v-model.number="metadataBatchSize"
            type="number"
            min="1"
            step="25"
          />
        </label>
        <div class="typeOptions" aria-label="Entity types">
          <label class="modeToggle">
            <input v-model="metadataTypes.artist" type="checkbox" />
            <span class="modeLabel">Artists</span>
          </label>
          <label class="modeToggle">
            <input v-model="metadataTypes.album" type="checkbox" />
            <span class="modeLabel">Albums</span>
          </label>
          <label class="modeToggle">
            <input v-model="metadataTypes.track" type="checkbox" />
            <span class="modeLabel">Tracks</span>
          </label>
        </div>
      </div>
      <button
        class="triggerButton"
        :disabled="job.is_running || triggeringJobs[job.id]"
        @click="triggerJob(job.id)"
      >
        {{
          triggeringJobs[job.id]
            ? "Triggering..."
            : job.is_running
              ? "Running..."
              : "Run Now"
        }}
      </button>
      <button
        v-if="job.is_running"
        class="stopButton"
        :disabled="stoppingJobs[job.id]"
        @click="stopJob(job.id)"
      >
        {{ stoppingJobs[job.id] ? "Stopping..." : "Stop" }}
      </button>
    </div>

    <div v-if="triggerError" class="errorMessage triggerError">
      {{ triggerError }}
    </div>
    <div v-if="stopError" class="errorMessage triggerError">
      {{ stopError }}
    </div>

    <h2 class="sectionTitle auditTitle">Job audit log</h2>

    <div v-if="auditLoading" class="loadingMessage">Loading audit log...</div>
    <div v-else-if="auditError" class="errorMessage">{{ auditError }}</div>
    <div v-else-if="auditEntries.length === 0" class="emptyMessage">
      No audit log entries
    </div>

    <div
      v-else
      class="auditTable"
      tabindex="0"
      role="region"
      aria-label="Job audit log"
    >
      <div class="auditHeader">
        <span class="auditCol time">Time</span>
        <span class="auditCol job">Job</span>
        <span class="auditCol event">Event</span>
        <span class="auditCol duration">Duration</span>
        <span class="auditCol details">Details</span>
      </div>
      <div v-for="entry in auditEntries" :key="entry.id" class="auditRow">
        <span class="auditCol time">{{
          formatTimestamp(entry.timestamp)
        }}</span>
        <span class="auditCol job">{{ entry.job_id }}</span>
        <span class="auditCol event">
          <span :class="['eventBadge', entry.event_type]">{{
            entry.event_type
          }}</span>
        </span>
        <span class="auditCol duration">{{
          formatDuration(entry.duration_ms)
        }}</span>
        <span class="auditCol details">
          <span v-if="entry.error" class="errorText">{{ entry.error }}</span>
          <span v-else-if="entry.details" class="detailsText">{{
            formatDetails(entry.details)
          }}</span>
          <span v-else class="noDetails">-</span>
        </span>
      </div>
    </div>

    <button
      v-if="auditEntries.length > 0"
      class="refreshButton"
      @click="loadAuditLog"
    >
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

const storageReport = ref(null);
const storageLoading = ref(true);
const storageError = ref(null);

// Relevance filter state
const filterLoading = ref(true);
const filterError = ref(null);
const filterMethod = ref("none");
const filterParams = reactive({
  threshold: 0.4,
  drop_threshold: 0.5,
  num_std_devs: 1.5,
  min_best_score: 5000,
});
const filterSaving = ref(false);
const filterSaveSuccess = ref(false);
const filterSaveError = ref(null);

// Background jobs state
const jobs = ref([]);
const jobsLoading = ref(true);
const jobsError = ref(null);
const triggeringJobs = reactive({});
const stoppingJobs = reactive({});
const triggerError = ref(null);
const stopError = ref(null);
const embeddingCoverage = ref(null);
const embeddingCoverageLoading = ref(true);
const embeddingCoverageError = ref(null);

// Job-specific options
const expandArtistsMode = ref("dry_run"); // "dry_run" or "actual"
const missingFilesMode = ref("dry_run"); // "dry_run" or "actual"
const embeddingMaxTracks = ref(1000);
const embeddingForce = ref(false);
const metadataBatchSize = ref(25);
const metadataTypes = reactive({
  artist: true,
  album: true,
  track: true,
});

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

const loadStorageReport = async () => {
  storageLoading.value = true;
  storageError.value = null;
  const data = await remoteStore.fetchStorageReport();
  if (data === null) {
    storageError.value = "Failed to load storage usage";
  } else {
    storageReport.value = data;
  }
  storageLoading.value = false;
};

const loadRelevanceFilter = async () => {
  filterLoading.value = true;
  filterError.value = null;
  const data = await remoteStore.fetchRelevanceFilter();
  if (data === null) {
    filterError.value = "Failed to load relevance filter";
  } else {
    filterMethod.value = data.config.method;
    if (data.config.threshold !== undefined)
      filterParams.threshold = data.config.threshold;
    if (data.config.drop_threshold !== undefined)
      filterParams.drop_threshold = data.config.drop_threshold;
    if (data.config.num_std_devs !== undefined)
      filterParams.num_std_devs = data.config.num_std_devs;
    if (data.config.min_best_score !== undefined)
      filterParams.min_best_score = data.config.min_best_score;
  }
  filterLoading.value = false;
};

const onFilterMethodChange = () => {
  filterSaveSuccess.value = false;
  filterSaveError.value = null;
};

const saveFilter = async () => {
  filterSaving.value = true;
  filterSaveSuccess.value = false;
  filterSaveError.value = null;

  let config = { method: filterMethod.value };
  switch (filterMethod.value) {
    case "percentage_of_best":
      config.threshold = filterParams.threshold;
      break;
    case "gap_detection":
      config.drop_threshold = filterParams.drop_threshold;
      break;
    case "standard_deviation":
      config.num_std_devs = filterParams.num_std_devs;
      break;
    case "percentage_with_minimum":
      config.threshold = filterParams.threshold;
      config.min_best_score = filterParams.min_best_score;
      break;
  }

  const result = await remoteStore.updateRelevanceFilter(config);
  if (result.success) {
    filterSaveSuccess.value = true;
    setTimeout(() => {
      filterSaveSuccess.value = false;
    }, 2000);
  } else {
    filterSaveError.value = result.error;
  }
  filterSaving.value = false;
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

const loadEmbeddingCoverage = async () => {
  embeddingCoverageLoading.value = true;
  embeddingCoverageError.value = null;
  const data = await remoteStore.fetchAudioEmbeddingCoverage();
  if (data === null) {
    embeddingCoverageError.value = "Failed to load embedding coverage";
  } else {
    embeddingCoverage.value = data;
  }
  embeddingCoverageLoading.value = false;
};

const triggerJob = async (jobId) => {
  triggeringJobs[jobId] = true;
  triggerError.value = null;

  // Build job-specific params
  let params = null;
  if (jobId === "expand_artists_base") {
    params = { mode: expandArtistsMode.value };
  } else if (jobId === "missing_files_watchdog") {
    params = { mode: missingFilesMode.value };
  } else if (jobId === "track_embedding_sync") {
    params = {
      max_tracks: Math.max(1, Number(embeddingMaxTracks.value) || 1000),
      force: embeddingForce.value,
    };
  } else if (jobId === "metadata_enrichment_v1") {
    const entityTypes = Object.entries(metadataTypes)
      .filter(([, enabled]) => enabled)
      .map(([entityType]) => entityType);
    params = {
      batch_size: Math.max(1, Number(metadataBatchSize.value) || 25),
      entity_types:
        entityTypes.length > 0 ? entityTypes : ["artist", "album", "track"],
    };
  }

  const result = await remoteStore.triggerBackgroundJob(jobId, params);

  if (result.error) {
    triggerError.value = `Failed to trigger job: ${result.error}`;
  } else {
    // Refresh job list to show updated status
    await loadJobs();
    // Refresh audit log to show the new entry
    await loadAuditLog();
    if (jobId === "track_embedding_sync") {
      await loadEmbeddingCoverage();
    }
  }

  triggeringJobs[jobId] = false;
};

const stopJob = async (jobId) => {
  stoppingJobs[jobId] = true;
  stopError.value = null;

  const result = await remoteStore.cancelBackgroundJob(jobId);

  if (result.error) {
    stopError.value = `Failed to stop job: ${result.error}`;
  } else {
    await loadJobs();
    await loadAuditLog();
  }

  stoppingJobs[jobId] = false;
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

const formatNumber = (value) => {
  return Number(value || 0).toLocaleString();
};

const formatBytes = (bytes) => {
  const value = Number(bytes || 0);
  if (value < 1024) return `${value} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let scaled = value / 1024;
  let unitIndex = 0;
  while (scaled >= 1024 && unitIndex < units.length - 1) {
    scaled /= 1024;
    unitIndex += 1;
  }
  return `${scaled.toFixed(scaled >= 10 ? 1 : 2)} ${units[unitIndex]}`;
};

const storagePercent = (bytes) => {
  const total = Number(storageReport.value?.total_bytes || 0);
  const value = Number(bytes || 0);
  if (total <= 0 || value <= 0) return "0%";
  return `${Math.min(100, (value / total) * 100).toFixed(1)}%`;
};

const modelForNamespace = (namespace) => {
  const spec = embeddingCoverage.value?.specs?.find(
    (item) => item.namespace === namespace,
  );
  return spec ? spec.model : "unknown model";
};

const formatDetails = (details) => {
  if (!details) return "-";

  const parts = [];

  // MissingFilesWatchdog - has 'mode' and 'total_tracks_scanned' fields
  if (
    details.mode !== undefined &&
    details.total_tracks_scanned !== undefined
  ) {
    const modeLabel = details.mode === "actual" ? "Actual" : "Dry-run";
    const scanned = `Scanned: ${details.total_tracks_scanned} tracks, ${details.total_album_images_scanned} album imgs, ${details.total_artist_images_scanned} artist imgs`;
    if (details.is_clean) {
      parts.push(`${modeLabel}: ✓ No missing files`);
      parts.push(scanned);
    } else {
      const missing = [];
      if (details.missing_track_audio_count > 0)
        missing.push(`${details.missing_track_audio_count} tracks`);
      if (details.missing_album_images_count > 0)
        missing.push(`${details.missing_album_images_count} album images`);
      if (details.missing_artist_images_count > 0)
        missing.push(`${details.missing_artist_images_count} artist images`);
      parts.push(`${modeLabel}: Missing ${missing.join(", ")}`);
      parts.push(scanned);
      if (details.items_queued > 0)
        parts.push(`Queued: ${details.items_queued}`);
      if (details.items_skipped > 0)
        parts.push(`Skipped: ${details.items_skipped}`);
    }
  }
  // ExpandArtistsBase - has 'mode' and 'artists_without_related_count' fields
  else if (
    details.mode !== undefined &&
    details.artists_without_related_count !== undefined
  ) {
    const modeLabel = details.mode === "actual" ? "Actual" : "Dry-run";
    if (details.is_clean) {
      parts.push(`${modeLabel}: ✓ No enrichment needed`);
    } else {
      const enrichment = [];
      if (details.artists_without_related_count > 0)
        enrichment.push(
          `${details.artists_without_related_count} without related`,
        );
      if (details.orphan_related_artist_ids_count > 0)
        enrichment.push(
          `${details.orphan_related_artist_ids_count} orphan relations`,
        );
      parts.push(`${modeLabel}: ${enrichment.join(", ")}`);
      if (details.items_queued > 0)
        parts.push(`Queued: ${details.items_queued}`);
      if (details.items_skipped > 0)
        parts.push(`Skipped: ${details.items_skipped}`);
    }
  }
  // Legacy IntegrityWatchdog (no mode field) - for backward compatibility
  else if (details.is_clean !== undefined && details.mode === undefined) {
    if (details.is_clean) {
      parts.push("✓ Catalog clean");
    } else {
      if (details.total_missing > 0) {
        const missing = [];
        if (details.missing_track_audio_count > 0)
          missing.push(`${details.missing_track_audio_count} tracks`);
        if (details.missing_album_images_count > 0)
          missing.push(`${details.missing_album_images_count} album images`);
        if (details.missing_artist_images_count > 0)
          missing.push(`${details.missing_artist_images_count} artist images`);
        parts.push(`Missing: ${missing.join(", ")}`);
      }
    }
    if (details.items_queued > 0) parts.push(`Queued: ${details.items_queued}`);
    if (details.items_skipped > 0)
      parts.push(`Skipped: ${details.items_skipped}`);
  }

  // PopularContent - started
  if (details.start_date !== undefined && details.end_date !== undefined) {
    parts.push(`Date range: ${details.start_date} - ${details.end_date}`);
    if (details.lookback_days)
      parts.push(`${details.lookback_days} day lookback`);
  }

  // PopularContent - completed
  if (
    details.albums_count !== undefined &&
    details.artists_count !== undefined
  ) {
    parts.push(
      `${details.albums_count} albums, ${details.artists_count} artists`,
    );
    if (details.tracks_analyzed)
      parts.push(`${details.tracks_analyzed} tracks analyzed`);
  }

  // PopularContent - skipped
  if (details.skipped) {
    parts.push(
      `Skipped: ${details.reason === "no_listening_data" ? "No listening data" : details.reason}`,
    );
  }

  // AuditLogCleanup - started
  if (details.retention_days !== undefined && parts.length === 0) {
    parts.push(`Retention: ${details.retention_days} days`);
  }

  // RelatedArtistsEnrichment - started
  if (
    details.batch_size !== undefined &&
    details.similar_artists_limit !== undefined &&
    parts.length === 0
  ) {
    parts.push(
      `Batch: ${details.batch_size}, limit: ${details.similar_artists_limit} similar`,
    );
  }

  // MetadataEnrichment - started/completed
  if (
    details.batch_size !== undefined &&
    details.entity_types !== undefined &&
    parts.length === 0
  ) {
    const types = Array.isArray(details.entity_types)
      ? details.entity_types.join(", ")
      : String(details.entity_types);
    parts.push(`Batch: ${details.batch_size}`);
    parts.push(`Types: ${types}`);
    if (details.processed !== undefined)
      parts.push(`Processed: ${details.processed}`);
    if (details.retryable_failures !== undefined)
      parts.push(`Retryable: ${details.retryable_failures}`);
    if (details.reason === "provider_not_configured")
      parts.push("Provider not configured");
  }

  // RelatedArtistsEnrichment - completed
  if (details.phase1 !== undefined && details.phase2 !== undefined) {
    const p1 = details.phase1;
    const p2 = details.phase2;
    const p1Parts = [];
    if (p1.processed > 0) {
      p1Parts.push(`${p1.mbid_found} found`);
      if (p1.mbid_not_found > 0) p1Parts.push(`${p1.mbid_not_found} not found`);
      if (p1.errors > 0) p1Parts.push(`${p1.errors} errors`);
      parts.push(`MBID: ${p1.processed} processed (${p1Parts.join(", ")})`);
    } else {
      parts.push("MBID: nothing to process");
    }
    if (p2.processed > 0) {
      const p2Parts = [`${p2.enriched} enriched`];
      if (p2.errors > 0) p2Parts.push(`${p2.errors} errors`);
      parts.push(`Related: ${p2.processed} processed (${p2Parts.join(", ")})`);
    } else {
      parts.push("Related: nothing to process");
    }
  }

  // AuditLogCleanup - completed
  if (details.total_deleted !== undefined) {
    if (details.total_deleted === 0) {
      parts.push("No entries to clean up");
    } else {
      const deleted = [];
      if (details.download_entries_deleted > 0)
        deleted.push(`${details.download_entries_deleted} download`);
      if (details.job_entries_deleted > 0)
        deleted.push(`${details.job_entries_deleted} job`);
      parts.push(`Deleted: ${deleted.join(", ")} entries`);
    }
  }

  return parts.length > 0 ? parts.join(" • ") : "-";
};

onMounted(() => {
  loadStorageReport();
  loadRelevanceFilter();
  loadEmbeddingCoverage();
  loadJobs();
  loadAuditLog();
});
</script>

<style scoped>
.serverControl {
  width: 100%;
  min-width: 0;
  color: var(--text-base);
  color-scheme: dark;
}
.pageHeader {
  margin-bottom: 28px;
}
.pageTitle {
  margin: 0 0 10px;
  font-size: 32px;
  font-weight: 700;
  line-height: 1.2;
  letter-spacing: -0.03em;
}
.pageHeader p {
  margin: 0;
  font-size: 14px;
  line-height: 1.5;
  color: var(--text-subdued);
}
.sectionTitle {
  margin: 36px 0 20px;
  font-size: 24px;
  font-weight: 700;
  letter-spacing: -0.02em;
}
.controlCard,
.storageCard,
.embeddingCoverageCard,
.jobCard {
  background: #181818;
  border-radius: 8px;
  padding: 24px;
}
.controlCard,
.jobCard {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 20px;
}
.controlInfo,
.jobInfo {
  flex: 1 1 260px;
  min-width: 0;
}
.controlTitle,
.jobTitle {
  margin: 0 0 10px;
  font-size: 18px;
  font-weight: 700;
}
.controlDescription,
.jobDescription {
  margin: 0;
  font-size: 14px;
  line-height: 1.6;
  color: var(--text-subdued);
}
button {
  font: inherit;
  font-size: 13px;
  font-weight: 700;
  cursor: pointer;
}
button:disabled {
  opacity: 0.45;
  cursor: default;
}
button:focus-visible,
[tabindex]:focus-visible {
  outline: 2px solid #fff;
  outline-offset: 3px;
}
.rebootButton,
.stopButton,
.refreshButton,
.triggerButton {
  min-height: 40px;
  padding: 8px 20px;
  border: 1px solid #727272;
  border-radius: 999px;
  background: transparent;
  color: var(--text-base);
  flex-shrink: 0;
}
.rebootButton,
.stopButton {
  color: #f3727f;
}
.rebootButton:hover:not(:disabled),
.stopButton:hover:not(:disabled),
.refreshButton:hover:not(:disabled),
.triggerButton:hover:not(:disabled) {
  border-color: #fff;
  background: #ffffff0c;
}
.saveButton {
  min-height: 44px;
  padding: 0 24px;
  border: 0;
  border-radius: 999px;
  background: var(--spotify-green);
  color: #000;
}
.saveButton:hover:not(:disabled) {
  background: var(--spotify-green-hover);
}
.refreshButton {
  margin-top: 16px;
}
.refreshButton.small {
  margin: 0;
}
.storageHeader,
.coverageHeader {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 24px;
}
.storageHeader > div,
.coverageHeader > div {
  flex: 1 1 240px;
  min-width: 0;
}
.storageBody,
.coverageBody {
  display: flex;
  flex-direction: column;
  gap: 24px;
}
.storageStats,
.coverageStats {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 20px;
  padding-bottom: 24px;
  border-bottom: 1px solid var(--surface-border);
}
.statValue {
  display: block;
  color: var(--text-base);
  font-size: 28px;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
}
.statLabel {
  display: block;
  margin-top: 6px;
  font-size: 13px;
  color: var(--text-subdued);
}
.coverageStat.warning .statValue,
.coverageWarning {
  color: #f0bc65;
}
.storageRows,
.coverageNamespaces {
  display: flex;
  flex-direction: column;
  gap: 16px;
}
.storageRow {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(160px, 28%);
  align-items: center;
  gap: 24px;
}
.storageMeta,
.namespaceInfo {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 0;
}
.storageName,
.namespaceName {
  font-size: 14px;
  font-weight: 500;
  overflow-wrap: anywhere;
}
.storagePath,
.storageBreakdown,
.namespaceModel {
  font-size: 12px;
  line-height: 1.5;
  color: var(--text-subdued);
  overflow-wrap: anywhere;
}
.storageMeasure {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 80px;
  gap: 16px;
  align-items: center;
}
.storageBar {
  height: 4px;
  border-radius: 999px;
  background: #535353;
  overflow: hidden;
}
.storageBar span {
  display: block;
  height: 100%;
  border-radius: inherit;
  background: var(--spotify-green);
}
.storageSize {
  font-size: 13px;
  font-variant-numeric: tabular-nums;
  text-align: right;
  white-space: nowrap;
}
.namespaceRow {
  display: flex;
  flex-wrap: wrap;
  justify-content: space-between;
  gap: 12px 24px;
}
.namespaceCounts {
  display: flex;
  flex-direction: column;
  gap: 6px;
  color: var(--text-subdued);
  font-size: 12px;
  font-variant-numeric: tabular-nums;
  text-align: right;
}
.coverageWarning {
  font-size: 13px;
  line-height: 1.5;
  margin: 0;
}
.embeddingCoverageCard {
  margin-bottom: 16px;
}
.jobCard {
  margin-bottom: 8px;
}
.jobDescription {
  margin-bottom: 10px;
}
.jobMeta {
  color: var(--text-subdued);
  font-size: 12px;
  line-height: 1.5;
}
.jobStatus.running,
.outcome.success,
.saveSuccess {
  color: var(--spotify-green);
}
.outcome.failed,
.errorText,
.saveError {
  color: #f3727f;
}
.jobOptions {
  flex: 1 1 240px;
  min-width: 0;
}
.metadataOptions,
.embeddingOptions,
.typeOptions {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 12px 20px;
}
.modeToggle {
  display: flex;
  align-items: center;
  gap: 8px;
  cursor: pointer;
  font-size: 13px;
}
.modeLabel {
  color: var(--text-subdued);
  line-height: 1.5;
}
input[type="checkbox"] {
  width: 18px;
  height: 18px;
  flex-shrink: 0;
  accent-color: var(--spotify-green);
}
.numberOption {
  display: flex;
  align-items: center;
  gap: 12px;
  color: var(--text-subdued);
  font-size: 13px;
}
input:not([type="checkbox"]),
select {
  min-width: 0;
  max-width: 100%;
  min-height: 44px;
  padding: 10px 12px;
  background: #242424;
  color: var(--text-base);
  color-scheme: dark;
  border: 1px solid #727272;
  border-radius: 4px;
  font: inherit;
  font-size: 14px;
}
input:focus-visible,
select:focus-visible {
  outline: 2px solid #fff;
  outline-offset: 2px;
}
.numberOption input {
  width: 100px;
}
.searchSettings {
  flex-direction: column;
  align-items: stretch;
}
.searchSettings .controlInfo {
  flex: auto;
}
.filterConfig {
  width: 100%;
}
.filterRow {
  display: flex;
  align-items: center;
  gap: 12px 20px;
  flex-wrap: wrap;
  margin-bottom: 16px;
}
.filterLabel {
  width: 140px;
  font-size: 14px;
  color: var(--text-subdued);
}
.filterSelect {
  width: 260px;
}
.filterInput {
  width: 100px;
}
.filterHint {
  color: var(--text-subdued);
  font-size: 12px;
  line-height: 1.5;
}
.filterActions {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 16px;
  margin-top: 24px;
  font-size: 13px;
}
.loadingMessage,
.emptyMessage {
  padding: 24px;
  font-size: 14px;
  color: var(--text-subdued);
}
.errorMessage {
  padding: 12px 16px;
  margin-top: 16px;
  background: #f3727f12;
  color: #f3727f;
  border-radius: 4px;
  font-size: 14px;
  overflow-wrap: anywhere;
}
.auditTable {
  max-width: 100%;
  overflow-x: auto;
  border-radius: 4px;
  overscroll-behavior-x: contain;
}
.auditHeader,
.auditRow {
  display: grid;
  grid-template-columns: 160px 180px 100px 80px minmax(180px, 1fr);
  min-width: 800px;
  padding: 14px 16px;
  gap: 16px;
  border-bottom: 1px solid var(--surface-border);
  font-size: 13px;
}
.auditHeader {
  background: #181818;
  color: var(--text-subdued);
  font-weight: 500;
}
.auditRow:hover {
  background: #ffffff08;
}
.auditCol {
  min-width: 0;
  overflow-wrap: anywhere;
}
.auditCol.time,
.auditCol.duration {
  color: var(--text-subdued);
  font-variant-numeric: tabular-nums;
  font-size: 12px;
}
.eventBadge {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  color: var(--text-subdued);
  font-size: 12px;
  text-transform: capitalize;
}
.eventBadge::before {
  content: "";
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: currentColor;
  flex-shrink: 0;
}
.eventBadge.started,
.eventBadge.progress {
  color: var(--spotify-green);
}
.eventBadge.failed {
  color: #f3727f;
}
.detailsText,
.noDetails {
  color: var(--text-subdued);
}
@media (max-width: 900px) {
  .storageRow {
    grid-template-columns: minmax(0, 1fr);
    gap: 12px;
  }
}
@media (max-width: 600px) {
  .controlCard,
  .storageCard,
  .embeddingCoverageCard,
  .jobCard {
    padding: 20px 16px;
  }
  .storageStats,
  .coverageStats {
    grid-template-columns: minmax(0, 1fr);
    gap: 20px;
  }
  .filterRow {
    align-items: flex-start;
  }
  .filterLabel {
    width: 100%;
  }
  .filterSelect {
    width: 100%;
  }
  .namespaceCounts {
    text-align: left;
  }
  .numberOption {
    flex-wrap: wrap;
  }
}
</style>
