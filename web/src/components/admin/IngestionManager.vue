<template>
  <div class="ingestionManager">
    <h2 class="sectionTitle">Ingestion Manager</h2>

    <!-- Upload Section -->
    <div class="uploadSection">
      <div
        class="uploadDropzone"
        :class="{ 'dragging': isDragging }"
        @click="triggerFileInput"
        @dragover.prevent="isDragging = true"
        @dragleave="isDragging = false"
        @drop.prevent="onDrop"
      >
        <input
          ref="fileInput"
          type="file"
          accept=".mp3,.flac,.wav,.ogg,.m4a,.aac,.wma,.opus,.zip"
          @change="handleFileSelect"
          style="display: none"
        />
        <div class="dropzoneContent">
          <span class="dropzoneIcon">+</span>
          <span class="dropzoneText">
            Drag audio file here or <span class="browseLink">browse</span>
          </span>
          <span class="dropzoneHint">Supports MP3, FLAC, WAV, OGG, M4A, AAC, OPUS, or ZIP archive</span>
        </div>
      </div>

      <!-- Upload Progress -->
      <div v-if="uploadState.uploading" class="uploadProgress">
        <div class="progressBar">
          <div class="progressFill" :style="{ width: uploadState.progress + '%' }"></div>
        </div>
        <span class="progressText">Uploading {{ uploadState.filename }}...</span>
      </div>

      <div v-if="uploadState.error" class="uploadError">
        {{ uploadState.error }}
      </div>

      <div v-if="uploadState.success" class="uploadSuccess">
        {{ uploadState.success }}
      </div>
    </div>

    <!-- Stats Summary -->
    <div class="statsSummary">
      <span class="statItem">
        <strong>{{ stats.pending }}</strong> pending
      </span>
      <span class="statItem">
        <strong>{{ stats.processing }}</strong> processing
      </span>
      <span class="statItem warning">
        <strong>{{ stats.awaitingReview }}</strong> awaiting review
      </span>
      <span class="statItem success">
        <strong>{{ stats.completed }}</strong> completed
      </span>
      <span class="statItem danger">
        <strong>{{ stats.failed }}</strong> failed
      </span>
    </div>

    <!-- Tab Navigation -->
    <div class="tabNav">
      <button
        v-for="tab in tabs"
        :key="tab.id"
        class="tabButton"
        :class="{ active: activeTab === tab.id }"
        @click="activeTab = tab.id"
      >
        {{ tab.label }}
        <span v-if="tab.count !== undefined" class="tabCount">{{ tab.count }}</span>
      </button>
    </div>

    <!-- My Jobs Tab -->
    <div v-if="activeTab === 'myJobs'" class="tabContent">
      <div v-if="myJobs.length === 0" class="emptyState">
        No ingestion jobs yet.
      </div>
      <div v-else class="jobList">
        <div v-for="job in myJobs" :key="job.id" class="jobItem" :class="statusClass(job.status)">
          <div class="jobHeader">
            <div class="jobMain">
              <span class="jobFilename">{{ job.original_filename }}</span>
              <span class="statusBadge" :class="statusClass(job.status)">
                {{ formatStatus(job.status) }}
              </span>
            </div>
            <div class="jobActions">
              <button
                v-if="job.status === 'PENDING'"
                class="actionButton primary"
                @click="processJob(job.id)"
                :disabled="processingJobs[job.id]"
              >
                {{ processingJobs[job.id] ? "..." : "Process" }}
              </button>
              <button
                v-if="job.status === 'CONVERTING'"
                class="actionButton secondary"
                @click="convertJob(job.id)"
                :disabled="processingJobs[job.id]"
              >
                {{ processingJobs[job.id] ? "..." : "Convert" }}
              </button>
              <button
                class="actionButton danger"
                @click="deleteJob(job.id)"
                :disabled="processingJobs[job.id]"
                title="Delete job"
              >
                ✕
              </button>
            </div>
          </div>
          <div class="jobDetails">
            <span class="detailItem">
              <span class="detailLabel">Size:</span>
              <span class="detailValue">{{ formatBytes(job.file_size_bytes) }}</span>
            </span>
            <span v-if="job.duration_ms" class="detailItem">
              <span class="detailLabel">Duration:</span>
              <span class="detailValue">{{ formatDuration(job.duration_ms) }}</span>
            </span>
            <span v-if="job.matched_track_id" class="detailItem">
              <span class="detailLabel">Matched:</span>
              <span class="detailValue trackId">{{ job.matched_track_id }}</span>
            </span>
            <span v-if="job.match_confidence" class="detailItem">
              <span class="detailLabel">Confidence:</span>
              <span class="detailValue">{{ (job.match_confidence * 100).toFixed(0) }}%</span>
            </span>
            <span class="detailItem">
              <span class="detailLabel">Created:</span>
              <span class="detailValue">{{ formatDate(job.created_at) }}</span>
            </span>
          </div>
          <div v-if="job.error_message" class="jobError">
            {{ job.error_message }}
          </div>
        </div>
      </div>
    </div>

    <!-- Review Queue Tab -->
    <div v-if="activeTab === 'review'" class="tabContent">
      <div v-if="reviewItems.length === 0" class="emptyState">
        No items awaiting review.
      </div>
      <div v-else class="reviewList">
        <div v-for="item in reviewItems" :key="item.id" class="reviewItem">
          <div class="reviewHeader">
            <span class="reviewQuestion">{{ item.question }}</span>
          </div>
          <div class="reviewOptions">
            <button
              v-for="option in parseOptions(item.options)"
              :key="option.id"
              class="reviewOption"
              @click="resolveReview(item.job_id, option.id)"
              :disabled="resolvingReviews[item.job_id]"
            >
              <span class="optionLabel">{{ option.label }}</span>
              <span v-if="option.description" class="optionDesc">{{ option.description }}</span>
            </button>
            <button
              class="reviewOption noMatch"
              @click="resolveReview(item.job_id, 'no_match')"
              :disabled="resolvingReviews[item.job_id]"
            >
              <span class="optionLabel">No Match</span>
              <span class="optionDesc">This file doesn't match any option</span>
            </button>
          </div>
          <div class="reviewMeta">
            <span class="detailItem">
              <span class="detailLabel">Job:</span>
              <span class="detailValue">{{ item.job_id }}</span>
            </span>
            <span class="detailItem">
              <span class="detailLabel">Created:</span>
              <span class="detailValue">{{ formatDate(item.created_at) }}</span>
            </span>
          </div>
        </div>
      </div>
    </div>

    <!-- Refresh Button -->
    <button class="refreshButton" @click="loadData" :disabled="isLoading">
      {{ isLoading ? "Loading..." : "Refresh" }}
    </button>
  </div>
</template>

<script setup>
import { ref, reactive, computed, onMounted, onUnmounted } from "vue";
import { useRemoteStore } from "@/store/remote";

const remoteStore = useRemoteStore();

// State
const activeTab = ref("myJobs");
const isLoading = ref(false);
const isDragging = ref(false);
const fileInput = ref(null);

const myJobs = ref([]);
const reviewItems = ref([]);
const processingJobs = reactive({});
const resolvingReviews = reactive({});

const uploadState = reactive({
  uploading: false,
  progress: 0,
  filename: "",
  error: null,
  success: null,
});

const stats = computed(() => {
  const s = { pending: 0, processing: 0, awaitingReview: 0, completed: 0, failed: 0 };
  for (const job of myJobs.value) {
    switch (job.status) {
      case "PENDING": s.pending++; break;
      case "PROCESSING": s.processing++; break;
      case "AWAITING_REVIEW": s.awaitingReview++; break;
      case "COMPLETED": s.completed++; break;
      case "FAILED": s.failed++; break;
    }
  }
  return s;
});

const tabs = computed(() => [
  { id: "myJobs", label: "My Jobs", count: myJobs.value.length },
  { id: "review", label: "Review Queue", count: reviewItems.value.length },
]);

// File Upload
const triggerFileInput = () => {
  fileInput.value?.click();
};

const handleFileSelect = (event) => {
  const file = event.target.files?.[0];
  if (file) {
    uploadFile(file);
  }
};

const onDrop = (e) => {
  isDragging.value = false;
  const file = e.dataTransfer?.files[0];
  if (file) {
    uploadFile(file);
  }
};

const uploadFile = async (file) => {
  uploadState.uploading = true;
  uploadState.progress = 0;
  uploadState.filename = file.name;
  uploadState.error = null;
  uploadState.success = null;

  try {
    // Send file directly via FormData with real-time progress tracking
    const result = await remoteStore.uploadIngestionFile(
      file,
      null,
      null,
      (progress) => {
        uploadState.progress = progress;
      },
    );

    if (result.error) {
      uploadState.error = result.error;
    } else {
      uploadState.success = `Job created: ${result.job_id}`;
      await loadData();
    }
  } catch (error) {
    console.error("[Ingestion] Upload error:", error);
    uploadState.error = error.message || "Upload failed";
  } finally {
    uploadState.uploading = false;
    if (fileInput.value) {
      fileInput.value.value = "";
    }
  }
};

// Data Loading
const loadData = async () => {
  isLoading.value = true;

  try {
    const [jobsResult, reviewResult] = await Promise.all([
      remoteStore.fetchIngestionMyJobs(),
      remoteStore.fetchIngestionReviews(),
    ]);

    myJobs.value = Array.isArray(jobsResult) ? jobsResult : [];
    reviewItems.value = reviewResult?.items || [];
  } catch (error) {
    console.error("Failed to load ingestion data:", error);
  }

  isLoading.value = false;
};

// Job Actions
const processJob = async (jobId) => {
  processingJobs[jobId] = true;
  try {
    await remoteStore.processIngestionJob(jobId);
    await loadData();
  } catch (error) {
    console.error("Failed to process job:", error);
  }
  processingJobs[jobId] = false;
};

const convertJob = async (jobId) => {
  processingJobs[jobId] = true;
  try {
    await remoteStore.convertIngestionJob(jobId);
    await loadData();
  } catch (error) {
    console.error("Failed to convert job:", error);
  }
  processingJobs[jobId] = false;
};

const deleteJob = async (jobId) => {
  if (!confirm("Delete this ingestion job?")) return;
  processingJobs[jobId] = true;
  try {
    await remoteStore.deleteIngestionJob(jobId);
    await loadData();
  } catch (error) {
    console.error("Failed to delete job:", error);
  }
  processingJobs[jobId] = false;
};

// Review Actions
const resolveReview = async (jobId, selectedOption) => {
  resolvingReviews[jobId] = true;
  try {
    await remoteStore.resolveIngestionReview(jobId, selectedOption);
    await loadData();
  } catch (error) {
    console.error("Failed to resolve review:", error);
  }
  resolvingReviews[jobId] = false;
};

// Formatting
const formatStatus = (status) => {
  if (!status) return "";
  return status.toLowerCase().replace("_", " ");
};

const statusClass = (status) => {
  switch (status?.toUpperCase()) {
    case "COMPLETED": return "status-completed";
    case "PROCESSING": return "status-progress";
    case "CONVERTING": return "status-converting";
    case "PENDING": return "status-pending";
    case "FAILED": return "status-failed";
    case "AWAITING_REVIEW": return "status-review";
    default: return "";
  }
};

const formatBytes = (bytes) => {
  if (bytes == null || bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unitIndex = 0;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex++;
  }
  return `${value.toFixed(unitIndex > 0 ? 1 : 0)} ${units[unitIndex]}`;
};

const formatDuration = (ms) => {
  if (ms == null) return "";
  const secs = Math.floor(ms / 1000);
  const mins = Math.floor(secs / 60);
  const remainingSecs = secs % 60;
  return `${mins}:${remainingSecs.toString().padStart(2, "0")}`;
};

const formatDate = (timestamp) => {
  if (!timestamp) return "";
  const date = new Date(timestamp);
  return date.toLocaleString();
};

const parseOptions = (optionsStr) => {
  try {
    return JSON.parse(optionsStr) || [];
  } catch {
    return [];
  }
};

// Auto-refresh
const REFRESH_INTERVAL = 10000;
let refreshInterval = null;

onMounted(() => {
  loadData();
  refreshInterval = setInterval(loadData, REFRESH_INTERVAL);
});

onUnmounted(() => {
  if (refreshInterval) {
    clearInterval(refreshInterval);
  }
});
</script>

<style scoped>
.ingestionManager {
  width: 100%;
}

.sectionTitle {
  font-size: var(--text-2xl);
  font-weight: var(--font-bold);
  color: var(--text-base);
  margin: 0 0 var(--spacing-4) 0;
}

/* Upload Section */
.uploadSection {
  margin-bottom: var(--spacing-4);
}

.uploadDropzone {
  border: 2px dashed var(--border-subdued);
  border-radius: var(--radius-lg);
  padding: var(--spacing-8);
  text-align: center;
  transition: all var(--transition-fast);
  cursor: pointer;
}

.uploadDropzone:hover,
.uploadDropzone.dragging {
  border-color: var(--spotify-green);
  background-color: rgba(29, 185, 84, 0.05);
}

.dropzoneContent {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--spacing-2);
}


.dropzoneIcon {
  font-size: var(--text-4xl);
  color: var(--text-subdued);
}

.dropzoneText {
  color: var(--text-subdued);
  font-size: var(--text-sm);
}

.browseLink {
  color: var(--spotify-green);
  font-weight: var(--font-medium);
  text-decoration: underline;
}

.dropzoneHint {
  font-size: var(--text-xs);
  color: var(--text-subdued);
}

.uploadProgress {
  margin-top: var(--spacing-3);
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
}

.progressBar {
  flex: 1;
  height: 6px;
  background-color: var(--bg-highlight);
  border-radius: 3px;
  overflow: hidden;
}

.progressFill {
  height: 100%;
  background-color: var(--spotify-green);
  transition: width 0.3s ease;
}

.progressText {
  font-size: var(--text-sm);
  color: var(--text-subdued);
}

.uploadError {
  margin-top: var(--spacing-3);
  padding: var(--spacing-3);
  background-color: rgba(220, 38, 38, 0.1);
  border: 1px solid #dc2626;
  border-radius: var(--radius-md);
  color: #dc2626;
  font-size: var(--text-sm);
}

.uploadSuccess {
  margin-top: var(--spacing-3);
  padding: var(--spacing-3);
  background-color: rgba(34, 197, 94, 0.1);
  border: 1px solid #22c55e;
  border-radius: var(--radius-md);
  color: #22c55e;
  font-size: var(--text-sm);
}

/* Stats Summary */
.statsSummary {
  display: flex;
  flex-wrap: wrap;
  gap: var(--spacing-4);
  padding: var(--spacing-3) var(--spacing-4);
  background-color: var(--bg-elevated-base);
  border-radius: var(--radius-lg);
  margin-bottom: var(--spacing-4);
  font-size: var(--text-sm);
  color: var(--text-subdued);
}

.statItem strong {
  color: var(--text-base);
}

.statItem.success strong { color: #22c55e; }
.statItem.danger strong { color: #dc2626; }
.statItem.warning strong { color: #f97316; }

/* Tab Navigation */
.tabNav {
  display: flex;
  gap: var(--spacing-2);
  margin-bottom: var(--spacing-4);
  border-bottom: 1px solid var(--border-subdued);
  padding-bottom: var(--spacing-2);
}

.tabButton {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  padding: var(--spacing-2) var(--spacing-4);
  background: none;
  border: none;
  color: var(--text-subdued);
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  cursor: pointer;
  border-radius: var(--radius-md);
  transition: all var(--transition-fast);
}

.tabButton:hover {
  color: var(--text-base);
  background-color: var(--bg-highlight);
}

.tabButton.active {
  color: var(--text-base);
  background-color: var(--bg-elevated-base);
}

.tabCount {
  background-color: var(--bg-highlight);
  padding: 2px 8px;
  border-radius: var(--radius-full);
  font-size: var(--text-xs);
}

/* Tab Content */
.tabContent {
  min-height: 200px;
}

.emptyState {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 200px;
  color: var(--text-subdued);
}

/* Job List */
.jobList {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
}

.jobItem {
  background-color: var(--bg-elevated-base);
  border-radius: var(--radius-md);
  padding: var(--spacing-3) var(--spacing-4);
  border-left: 3px solid var(--border-subdued);
}

.jobItem.status-pending { border-left-color: #9ca3af; }
.jobItem.status-progress { border-left-color: #3b82f6; }
.jobItem.status-converting { border-left-color: #f59e0b; }
.jobItem.status-completed { border-left-color: #22c55e; }
.jobItem.status-failed { border-left-color: #dc2626; }
.jobItem.status-review { border-left-color: #f97316; }

.jobHeader {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: var(--spacing-2);
}

.jobMain {
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
}

.jobFilename {
  font-weight: var(--font-medium);
  color: var(--text-base);
}

.jobActions {
  display: flex;
  gap: var(--spacing-2);
}

.actionButton {
  padding: 4px 12px;
  border-radius: var(--radius-md);
  font-size: var(--text-xs);
  font-weight: var(--font-medium);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.actionButton.primary {
  background-color: var(--spotify-green);
  color: white;
  border: none;
}

.actionButton.primary:hover:not(:disabled) {
  background-color: #1ed760;
}

.actionButton.secondary {
  background-color: transparent;
  color: var(--text-subdued);
  border: 1px solid var(--border-subdued);
}

.actionButton.secondary:hover:not(:disabled) {
  border-color: var(--text-base);
  color: var(--text-base);
}

.actionButton.danger {
  background-color: transparent;
  color: var(--text-subdued);
  border: 1px solid var(--border-subdued);
  padding: 4px 8px;
}

.actionButton.danger:hover:not(:disabled) {
  border-color: #dc2626;
  color: #dc2626;
}

.actionButton:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.jobDetails {
  display: flex;
  flex-wrap: wrap;
  gap: var(--spacing-3);
  font-size: var(--text-xs);
}

.detailItem {
  display: inline-flex;
  gap: var(--spacing-1);
}

.detailLabel {
  color: var(--text-subdued);
}

.detailValue {
  color: var(--text-base);
}

.detailValue.trackId {
  font-family: monospace;
  font-size: var(--text-xs);
}

.jobError {
  margin-top: var(--spacing-2);
  font-size: var(--text-xs);
  color: #dc2626;
}

/* Status Badge */
.statusBadge {
  display: inline-block;
  padding: 2px 8px;
  border-radius: var(--radius-full);
  font-size: var(--text-xs);
  font-weight: var(--font-medium);
}

.status-completed { background-color: rgba(34, 197, 94, 0.15); color: #22c55e; }
.status-progress { background-color: rgba(59, 130, 246, 0.15); color: #3b82f6; }
.status-converting { background-color: rgba(245, 158, 11, 0.15); color: #f59e0b; }
.status-pending { background-color: rgba(156, 163, 175, 0.15); color: #9ca3af; }
.status-failed { background-color: rgba(220, 38, 38, 0.15); color: #dc2626; }
.status-review { background-color: rgba(249, 115, 22, 0.15); color: #f97316; }

/* Review List */
.reviewList {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-3);
}

.reviewItem {
  background-color: var(--bg-elevated-base);
  border-radius: var(--radius-md);
  padding: var(--spacing-4);
  border-left: 3px solid #f97316;
}

.reviewHeader {
  margin-bottom: var(--spacing-3);
}

.reviewQuestion {
  font-weight: var(--font-medium);
  color: var(--text-base);
}

.reviewOptions {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
  margin-bottom: var(--spacing-3);
}

.reviewOption {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  padding: var(--spacing-3);
  background-color: var(--bg-base);
  border: 1px solid var(--border-subdued);
  border-radius: var(--radius-md);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.reviewOption:hover:not(:disabled) {
  border-color: var(--spotify-green);
}

.reviewOption.noMatch {
  border-color: var(--border-subdued);
}

.reviewOption.noMatch:hover:not(:disabled) {
  border-color: #dc2626;
}

.reviewOption:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.optionLabel {
  font-weight: var(--font-medium);
  color: var(--text-base);
}

.optionDesc {
  font-size: var(--text-xs);
  color: var(--text-subdued);
  margin-top: var(--spacing-1);
}

.reviewMeta {
  display: flex;
  gap: var(--spacing-3);
  font-size: var(--text-xs);
}

/* Refresh Button */
.refreshButton {
  margin-top: var(--spacing-4);
  padding: var(--spacing-2) var(--spacing-4);
  background-color: var(--bg-elevated-base);
  border: 1px solid var(--border-subdued);
  border-radius: var(--radius-md);
  color: var(--text-subdued);
  font-size: var(--text-sm);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.refreshButton:hover:not(:disabled) {
  border-color: var(--text-base);
  color: var(--text-base);
}

.refreshButton:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}
</style>
