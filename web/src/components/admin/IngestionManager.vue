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
        <input
          ref="folderInput"
          type="file"
          webkitdirectory
          directory
          @change="handleFolderSelect"
          style="display: none"
        />
        <div class="dropzoneContent">
          <span class="dropzoneIcon">+</span>
          <span class="dropzoneText">
            Drag files/folders here or <span class="browseLink">browse files</span>
          </span>
          <span class="dropzoneHint">Supports MP3, FLAC, WAV, OGG, M4A, AAC, OPUS, ZIP, or folders</span>
          <button class="folderButton" @click.stop="triggerFolderInput">
            Select Folder
          </button>
        </div>
      </div>

      <!-- Upload Progress -->
      <div v-if="uploadState.uploading || uploadState.zipping" class="uploadProgress">
        <div class="progressBar">
          <div class="progressFill" :style="{ width: uploadState.progress + '%' }"></div>
        </div>
        <span class="progressText">
          {{ uploadState.zipping ? 'Zipping' : 'Uploading' }} {{ uploadState.filename }}...
        </span>
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
              <span v-if="job.ticket_type" class="ticketBadge" :class="ticketClass(job.ticket_type)">
                {{ job.ticket_type }}
              </span>
              <span v-if="job.upload_type" class="uploadTypeBadge">
                {{ job.upload_type }}
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
              <span class="detailLabel">Files:</span>
              <span class="detailValue">{{ job.file_count }}</span>
            </span>
            <span class="detailItem">
              <span class="detailLabel">Size:</span>
              <span class="detailValue">{{ formatBytes(job.total_size_bytes) }}</span>
            </span>
            <span v-if="job.matched_album_id" class="detailItem">
              <span class="detailLabel">Album:</span>
              <span class="detailValue">{{ job.detected_album || job.matched_album_id }}</span>
            </span>
            <span v-if="job.detected_artist" class="detailItem">
              <span class="detailLabel">Artist:</span>
              <span class="detailValue">{{ job.detected_artist }}</span>
            </span>
            <span v-if="job.match_score != null" class="detailItem">
              <span class="detailLabel">Match:</span>
              <span class="detailValue">{{ (job.match_score * 100).toFixed(0) }}%</span>
            </span>
            <span v-if="job.match_delta_ms != null" class="detailItem">
              <span class="detailLabel">Delta:</span>
              <span class="detailValue">{{ job.match_delta_ms }}ms</span>
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
import { useIngestionStore } from "@/store/ingestion";
import JSZip from "jszip";

const remoteStore = useRemoteStore();
const ingestionStore = useIngestionStore();

// Supported audio extensions
const AUDIO_EXTENSIONS = ["mp3", "flac", "wav", "ogg", "m4a", "aac", "wma", "opus"];

// State
const activeTab = ref("myJobs");
const isLoading = ref(false);
const isDragging = ref(false);
const fileInput = ref(null);
const folderInput = ref(null);

const myJobs = ref([]);
const reviewItems = ref([]);
const processingJobs = reactive({});
const resolvingReviews = reactive({});

const uploadState = reactive({
  uploading: false,
  zipping: false,
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

const triggerFolderInput = () => {
  folderInput.value?.click();
};

const handleFileSelect = (event) => {
  const file = event.target.files?.[0];
  if (file) {
    uploadFile(file);
  }
};

const handleFolderSelect = async (event) => {
  const files = event.target.files;
  if (files && files.length > 0) {
    await uploadFolder(files);
  }
};

const onDrop = async (e) => {
  isDragging.value = false;

  // Check if it's a folder drop using DataTransferItemList
  const items = e.dataTransfer?.items;
  if (items && items.length > 0) {
    const firstItem = items[0];

    // Try to get as directory entry (for folder drops)
    if (firstItem.webkitGetAsEntry) {
      const entry = firstItem.webkitGetAsEntry();
      if (entry && entry.isDirectory) {
        await uploadDirectoryEntry(entry);
        return;
      }
    }
  }

  // Fall back to regular file upload
  const file = e.dataTransfer?.files[0];
  if (file) {
    uploadFile(file);
  }
};

// Check if a file is a supported audio format
const isAudioFile = (filename) => {
  const ext = filename.split('.').pop()?.toLowerCase();
  return AUDIO_EXTENSIONS.includes(ext);
};

// Upload a folder by zipping it first
const uploadFolder = async (files) => {
  // Filter to only audio files
  const audioFiles = Array.from(files).filter(f => isAudioFile(f.name));

  if (audioFiles.length === 0) {
    uploadState.error = "No audio files found in folder";
    return;
  }

  // Get folder name from webkitRelativePath
  const folderName = audioFiles[0].webkitRelativePath?.split('/')[0] || 'folder';

  uploadState.zipping = true;
  uploadState.progress = 0;
  uploadState.filename = folderName;
  uploadState.error = null;
  uploadState.success = null;

  try {
    // Create zip
    const zip = new JSZip();

    for (let i = 0; i < audioFiles.length; i++) {
      const file = audioFiles[i];
      const relativePath = file.webkitRelativePath || file.name;
      zip.file(relativePath, file);
      uploadState.progress = Math.round((i / audioFiles.length) * 50);
    }

    // Generate zip blob
    const zipBlob = await zip.generateAsync(
      { type: "blob" },
      (metadata) => {
        uploadState.progress = 50 + Math.round(metadata.percent / 2);
      }
    );

    uploadState.zipping = false;
    uploadState.uploading = true;
    uploadState.progress = 0;

    // Upload the zip
    const zipFile = new File([zipBlob], `${folderName}.zip`, { type: "application/zip" });

    const result = await remoteStore.uploadIngestionFile(
      zipFile,
      null,
      null,
      (progress) => {
        uploadState.progress = progress;
      },
    );

    if (result.error) {
      uploadState.error = result.error;
    } else {
      const jobCount = result.job_ids?.length || 1;
      uploadState.success = jobCount > 1
        ? `Created ${jobCount} jobs from ${folderName}`
        : `Job created: ${result.job_id || result.job_ids?.[0]}`;
      // Add sessions to ingestion store and open monitor
      const jobIds = result.job_ids || (result.job_id ? [result.job_id] : []);
      for (const jobId of jobIds) {
        ingestionStore.addSession({
          id: jobId,
          status: "PENDING",
          original_filename: folderName,
        });
        ingestionStore.fetchJobDetails(jobId);
      }
      if (jobIds.length > 0) {
        ingestionStore.openModal(jobIds[0]);
      }
      await loadData();
    }
  } catch (error) {
    console.error("[Ingestion] Folder upload error:", error);
    uploadState.error = error.message || "Folder upload failed";
  } finally {
    uploadState.uploading = false;
    uploadState.zipping = false;
    if (folderInput.value) {
      folderInput.value.value = "";
    }
  }
};

// Upload a directory from drag & drop using webkitGetAsEntry
const uploadDirectoryEntry = async (dirEntry) => {
  const folderName = dirEntry.name;

  uploadState.zipping = true;
  uploadState.progress = 0;
  uploadState.filename = folderName;
  uploadState.error = null;
  uploadState.success = null;

  try {
    // Recursively read all files from the directory
    const files = await readDirectoryRecursive(dirEntry);
    const audioFiles = files.filter(f => isAudioFile(f.path));

    if (audioFiles.length === 0) {
      uploadState.error = "No audio files found in folder";
      uploadState.zipping = false;
      return;
    }

    // Create zip
    const zip = new JSZip();

    for (let i = 0; i < audioFiles.length; i++) {
      const { path, file } = audioFiles[i];
      zip.file(path, file);
      uploadState.progress = Math.round((i / audioFiles.length) * 50);
    }

    // Generate zip blob
    const zipBlob = await zip.generateAsync(
      { type: "blob" },
      (metadata) => {
        uploadState.progress = 50 + Math.round(metadata.percent / 2);
      }
    );

    uploadState.zipping = false;
    uploadState.uploading = true;
    uploadState.progress = 0;

    // Upload the zip
    const zipFile = new File([zipBlob], `${folderName}.zip`, { type: "application/zip" });

    const result = await remoteStore.uploadIngestionFile(
      zipFile,
      null,
      null,
      (progress) => {
        uploadState.progress = progress;
      },
    );

    if (result.error) {
      uploadState.error = result.error;
    } else {
      const jobCount = result.job_ids?.length || 1;
      uploadState.success = jobCount > 1
        ? `Created ${jobCount} jobs from ${folderName}`
        : `Job created: ${result.job_id || result.job_ids?.[0]}`;
      // Add sessions to ingestion store and open monitor
      const jobIds = result.job_ids || (result.job_id ? [result.job_id] : []);
      for (const jobId of jobIds) {
        ingestionStore.addSession({
          id: jobId,
          status: "PENDING",
          original_filename: folderName,
        });
        ingestionStore.fetchJobDetails(jobId);
      }
      if (jobIds.length > 0) {
        ingestionStore.openModal(jobIds[0]);
      }
      await loadData();
    }
  } catch (error) {
    console.error("[Ingestion] Directory upload error:", error);
    uploadState.error = error.message || "Directory upload failed";
  } finally {
    uploadState.uploading = false;
    uploadState.zipping = false;
  }
};

// Recursively read all files from a directory entry
const readDirectoryRecursive = async (dirEntry, basePath = "") => {
  const files = [];
  const entries = await readDirectoryEntries(dirEntry);

  for (const entry of entries) {
    const path = basePath ? `${basePath}/${entry.name}` : entry.name;

    if (entry.isFile) {
      const file = await getFileFromEntry(entry);
      files.push({ path, file });
    } else if (entry.isDirectory) {
      const subFiles = await readDirectoryRecursive(entry, path);
      files.push(...subFiles);
    }
  }

  return files;
};

// Read all entries from a directory
const readDirectoryEntries = (dirEntry) => {
  return new Promise((resolve, reject) => {
    const reader = dirEntry.createReader();
    const entries = [];

    const readBatch = () => {
      reader.readEntries((batch) => {
        if (batch.length === 0) {
          resolve(entries);
        } else {
          entries.push(...batch);
          readBatch();
        }
      }, reject);
    };

    readBatch();
  });
};

// Get File object from FileEntry
const getFileFromEntry = (fileEntry) => {
  return new Promise((resolve, reject) => {
    fileEntry.file(resolve, reject);
  });
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
      const jobIds = result.job_ids || (result.job_id ? [result.job_id] : []);
      const jobCount = jobIds.length;
      uploadState.success = jobCount > 1
        ? `Created ${jobCount} jobs from ${file.name}`
        : `Job created: ${jobIds[0]}`;
      // Add sessions to ingestion store and open monitor
      for (const jobId of jobIds) {
        ingestionStore.addSession({
          id: jobId,
          status: "PENDING",
          original_filename: file.name,
        });
        ingestionStore.fetchJobDetails(jobId);
      }
      if (jobIds.length > 0) {
        ingestionStore.openModal(jobIds[0]);
      }
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
    case "ANALYZING": return "status-progress";
    case "IDENTIFYING_ALBUM": return "status-progress";
    case "MAPPING_TRACKS": return "status-progress";
    case "CONVERTING": return "status-converting";
    case "PENDING": return "status-pending";
    case "FAILED": return "status-failed";
    case "AWAITING_REVIEW": return "status-review";
    default: return "";
  }
};

const ticketClass = (ticketType) => {
  switch (ticketType?.toUpperCase()) {
    case "SUCCESS": return "ticket-success";
    case "REVIEW": return "ticket-review";
    case "FAILURE": return "ticket-failure";
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

.folderButton {
  margin-top: var(--spacing-2);
  padding: var(--spacing-2) var(--spacing-4);
  background-color: var(--bg-elevated-base);
  border: 1px solid var(--border-subdued);
  border-radius: var(--radius-md);
  color: var(--text-subdued);
  font-size: var(--text-sm);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.folderButton:hover {
  border-color: var(--spotify-green);
  color: var(--spotify-green);
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

/* Ticket Type Badge */
.ticketBadge {
  display: inline-block;
  padding: 2px 8px;
  border-radius: var(--radius-full);
  font-size: var(--text-xs);
  font-weight: var(--font-medium);
  text-transform: uppercase;
}

.ticket-success { background-color: rgba(34, 197, 94, 0.2); color: #22c55e; }
.ticket-review { background-color: rgba(249, 115, 22, 0.2); color: #f97316; }
.ticket-failure { background-color: rgba(220, 38, 38, 0.2); color: #dc2626; }

/* Upload Type Badge */
.uploadTypeBadge {
  display: inline-block;
  padding: 2px 8px;
  border-radius: var(--radius-full);
  font-size: var(--text-xs);
  font-weight: var(--font-medium);
  background-color: rgba(139, 92, 246, 0.15);
  color: #8b5cf6;
  text-transform: lowercase;
}

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
