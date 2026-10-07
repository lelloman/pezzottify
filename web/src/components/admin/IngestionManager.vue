<template>
  <div class="ingestionManager">
    <header class="pageHeader">
      <div>
        <h2 class="sectionTitle">Ingestion</h2>
        <p>Upload audio, follow processing and review catalog matches.</p>
      </div>
      <button class="refreshButton" @click="loadData" :disabled="isLoading">
        {{ isLoading ? "Loading…" : "Refresh" }}
      </button>
    </header>

    <!-- Upload Section -->
    <div class="uploadSection">
      <div
        class="uploadDropzone"
        role="button"
        tabindex="0"
        aria-label="Choose audio files"
        @keydown.enter.self="triggerFileInput"
        @keydown.space.self.prevent="triggerFileInput"
        :class="{ dragging: isDragging }"
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
          <svg class="dropzoneIcon" viewBox="0 0 24 24" aria-hidden="true">
            <path d="M12 16V3m-5 5 5-5 5 5M4 15v5h16v-5" />
          </svg>
          <span class="dropzoneText">
            Drag files/folders here or
            <span class="browseLink">browse files</span>
          </span>
          <span class="dropzoneHint"
            >Supports MP3, FLAC, WAV, OGG, M4A, AAC, OPUS, ZIP, or folders</span
          >
          <button class="folderButton" @click.stop="triggerFolderInput">
            Select Folder
          </button>
        </div>
      </div>

      <!-- Upload Progress -->
      <div
        v-if="uploadState.uploading || uploadState.zipping"
        class="uploadProgress"
      >
        <div class="progressBar">
          <div
            class="progressFill"
            :style="{ width: uploadState.progress + '%' }"
          ></div>
        </div>
        <span class="progressText">
          {{ uploadState.zipping ? "Zipping" : "Uploading" }}
          {{ uploadState.filename }}...
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
        :aria-pressed="activeTab === tab.id"
        @click="activeTab = tab.id"
      >
        {{ tab.label }}
        <span v-if="tab.count !== undefined" class="tabCount">{{
          tab.count
        }}</span>
      </button>
    </div>

    <!-- My Jobs Tab -->
    <div v-if="activeTab === 'myJobs'" class="tabContent">
      <div v-if="myJobs.length === 0" class="emptyState">
        No ingestion jobs yet.
      </div>
      <div v-else class="jobList">
        <div
          v-for="job in myJobs"
          :key="job.id"
          class="jobItem"
          :class="statusClass(job.status)"
        >
          <div class="jobHeader">
            <div class="jobMain">
              <span class="jobFilename">{{ job.original_filename }}</span>
              <span class="statusBadge" :class="statusClass(job.status)">
                {{ formatStatus(job.status) }}
              </span>
              <span
                v-if="job.ticket_type"
                class="ticketBadge"
                :class="ticketClass(job.ticket_type)"
              >
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
                :aria-label="`Delete ${job.original_filename}`"
              >
                <svg viewBox="0 0 24 24" aria-hidden="true">
                  <path
                    d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13M10 10v7M14 10v7"
                  />
                </svg>
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
              <span class="detailValue">{{
                formatBytes(job.total_size_bytes)
              }}</span>
            </span>
            <span v-if="job.matched_album_id" class="detailItem">
              <span class="detailLabel">Album:</span>
              <span class="detailValue">{{
                job.detected_album || job.matched_album_id
              }}</span>
            </span>
            <span v-if="job.detected_artist" class="detailItem">
              <span class="detailLabel">Artist:</span>
              <span class="detailValue">{{ job.detected_artist }}</span>
            </span>
            <span v-if="job.match_score != null" class="detailItem">
              <span class="detailLabel">Match:</span>
              <span class="detailValue"
                >{{ (job.match_score * 100).toFixed(0) }}%</span
              >
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

          <!-- Album comparison table -->
          <template v-if="reviewAlbumData[item.job_id]">
            <div class="reviewAlbumHeader">
              <div class="reviewAlbumInfo">
                <span class="reviewAlbumArtist">{{
                  reviewAlbumData[item.job_id].artistName
                }}</span>
                <span class="reviewAlbumName">{{
                  reviewAlbumData[item.job_id].albumName
                }}</span>
              </div>
              <div class="reviewAlbumScores">
                <span
                  class="scoreBadge"
                  :class="fpScoreClass(reviewAlbumData[item.job_id].fpScore)"
                  >FP {{ reviewAlbumData[item.job_id].fpScore ?? "?" }}%</span
                >
                <span
                  v-if="reviewAlbumData[item.job_id].metaScore != null"
                  class="scoreBadge scoreMeta"
                  >Meta {{ reviewAlbumData[item.job_id].metaScore }}%</span
                >
                <span class="reviewTrackCount">
                  {{ reviewAlbumData[item.job_id].catalogTracks.length }}
                  catalog /
                  {{ reviewAlbumData[item.job_id].uploadedFiles.length }}
                  uploaded
                </span>
              </div>
            </div>
            <div
              class="trackTableWrapper"
              tabindex="0"
              role="region"
              aria-label="Catalog and uploaded track comparison"
            >
              <table class="trackTable">
                <thead>
                  <tr>
                    <th class="colNum">#</th>
                    <th class="colName">Catalog Track</th>
                    <th class="colDur">Duration</th>
                    <th class="colDelta">Delta</th>
                    <th class="colDur">Duration</th>
                    <th class="colName">Uploaded File</th>
                  </tr>
                </thead>
                <tbody>
                  <tr
                    v-for="(row, i) in buildComparisonRows(
                      reviewAlbumData[item.job_id],
                    )"
                    :key="i"
                  >
                    <td class="colNum">{{ row.number }}</td>
                    <td class="colName" :title="row.catalogName">
                      {{ row.catalogName || "—" }}
                    </td>
                    <td class="colDur mono">{{ row.catalogDuration }}</td>
                    <td class="colDelta mono" :class="row.deltaClass">
                      {{ row.delta }}
                    </td>
                    <td class="colDur mono">{{ row.uploadedDuration }}</td>
                    <td class="colName" :title="row.uploadedName">
                      {{ row.uploadedName || "—" }}
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>
          </template>
          <div
            v-else-if="reviewAlbumLoading[item.job_id]"
            class="reviewAlbumLoading"
          >
            Loading album comparison...
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
              <span v-if="option.description" class="optionDesc">{{
                option.description
              }}</span>
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
const AUDIO_EXTENSIONS = [
  "mp3",
  "flac",
  "wav",
  "ogg",
  "m4a",
  "aac",
  "wma",
  "opus",
];

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
const reviewAlbumData = reactive({});
const reviewAlbumLoading = reactive({});

const uploadState = reactive({
  uploading: false,
  zipping: false,
  progress: 0,
  filename: "",
  error: null,
  success: null,
});

const stats = computed(() => {
  const s = {
    pending: 0,
    processing: 0,
    awaitingReview: 0,
    completed: 0,
    failed: 0,
  };
  for (const job of myJobs.value) {
    switch (job.status) {
      case "PENDING":
        s.pending++;
        break;
      case "PROCESSING":
        s.processing++;
        break;
      case "AWAITING_REVIEW":
        s.awaitingReview++;
        break;
      case "COMPLETED":
        s.completed++;
        break;
      case "FAILED":
        s.failed++;
        break;
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
  const ext = filename.split(".").pop()?.toLowerCase();
  return AUDIO_EXTENSIONS.includes(ext);
};

// Upload a folder by zipping it first
const uploadFolder = async (files) => {
  // Filter to only audio files
  const audioFiles = Array.from(files).filter((f) => isAudioFile(f.name));

  if (audioFiles.length === 0) {
    uploadState.error = "No audio files found in folder";
    return;
  }

  // Get folder name from webkitRelativePath
  const folderName =
    audioFiles[0].webkitRelativePath?.split("/")[0] || "folder";

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
    const zipBlob = await zip.generateAsync({ type: "blob" }, (metadata) => {
      uploadState.progress = 50 + Math.round(metadata.percent / 2);
    });

    uploadState.zipping = false;
    uploadState.uploading = true;
    uploadState.progress = 0;

    // Upload the zip
    const zipFile = new File([zipBlob], `${folderName}.zip`, {
      type: "application/zip",
    });

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
      uploadState.success =
        jobCount > 1
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
    const audioFiles = files.filter((f) => isAudioFile(f.path));

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
    const zipBlob = await zip.generateAsync({ type: "blob" }, (metadata) => {
      uploadState.progress = 50 + Math.round(metadata.percent / 2);
    });

    uploadState.zipping = false;
    uploadState.uploading = true;
    uploadState.progress = 0;

    // Upload the zip
    const zipFile = new File([zipBlob], `${folderName}.zip`, {
      type: "application/zip",
    });

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
      uploadState.success =
        jobCount > 1
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
      uploadState.success =
        jobCount > 1
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

    // Fetch album comparison data for review items (non-blocking)
    if (reviewItems.value.length > 0) {
      loadReviewAlbumData(reviewItems.value);
    }
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

// Review album comparison
const loadReviewAlbumData = async (items) => {
  for (const item of items) {
    if (reviewAlbumData[item.job_id] || reviewAlbumLoading[item.job_id])
      continue;

    // Parse options to find album candidate
    const options = parseOptions(item.options);
    const albumOption = options.find((o) => o.id.startsWith("album:"));
    if (!albumOption) continue;

    const albumId = albumOption.id.substring(6);
    // Parse scores from label: "Artist - Album (fingerprint XX%, metadata YY%)"
    const fpMatch = albumOption.label.match(/fingerprint\s+(\d+)%/);
    const metaMatch = albumOption.label.match(/metadata\s+(\d+)%/);

    reviewAlbumLoading[item.job_id] = true;

    // Fetch job details (for uploaded files) and resolved album (for catalog tracks) in parallel
    const [jobDetails, resolvedAlbum] = await Promise.all([
      remoteStore.fetchIngestionJobDetails(item.job_id),
      remoteStore.fetchResolvedAlbum(albumId),
    ]);

    if (resolvedAlbum && jobDetails?.files) {
      // Flatten catalog tracks sorted by disc/track number
      const catalogTracks = [];
      for (const disc of resolvedAlbum.discs || []) {
        for (const track of disc.tracks || []) {
          catalogTracks.push({
            discNumber: disc.number,
            trackNumber: track.track_number,
            name: track.name,
            durationMs: track.duration_ms,
          });
        }
      }
      catalogTracks.sort(
        (a, b) => a.discNumber - b.discNumber || a.trackNumber - b.trackNumber,
      );

      // Sort uploaded files by disc/track tags then filename
      const uploadedFiles = [...jobDetails.files].sort((a, b) => {
        const dA = a.tag_disc_num || 1;
        const dB = b.tag_disc_num || 1;
        if (dA !== dB) return dA - dB;
        const tA = a.tag_track_num || 999;
        const tB = b.tag_track_num || 999;
        if (tA !== tB) return tA - tB;
        return (a.filename || "").localeCompare(b.filename || "");
      });

      reviewAlbumData[item.job_id] = {
        albumName: resolvedAlbum.album?.name || "Unknown Album",
        artistName:
          resolvedAlbum.artists?.map((a) => a.name).join(", ") ||
          "Unknown Artist",
        fpScore: fpMatch ? parseInt(fpMatch[1]) : null,
        metaScore: metaMatch ? parseInt(metaMatch[1]) : null,
        catalogTracks,
        uploadedFiles,
      };
    }

    reviewAlbumLoading[item.job_id] = false;
  }
};

const buildComparisonRows = (data) => {
  if (!data) return [];
  const rows = [];
  const maxLen = Math.max(data.catalogTracks.length, data.uploadedFiles.length);
  for (let i = 0; i < maxLen; i++) {
    const catalog = data.catalogTracks[i];
    const uploaded = data.uploadedFiles[i];

    const catalogMs = catalog?.durationMs;
    const uploadedMs = uploaded?.duration_ms;

    let deltaClass = "";
    let deltaStr = "—";
    if (catalogMs != null && uploadedMs != null) {
      const deltaMs = Math.abs(uploadedMs - catalogMs);
      const deltaSec = deltaMs / 1000;
      if (deltaSec < 2) deltaClass = "deltaOk";
      else if (deltaSec < 5) deltaClass = "deltaWarn";
      else deltaClass = "deltaBad";
      deltaStr = deltaSec < 1 ? `${deltaMs}ms` : `${deltaSec.toFixed(1)}s`;
    }

    rows.push({
      number: catalog
        ? `${catalog.discNumber > 1 ? catalog.discNumber + "." : ""}${catalog.trackNumber}`
        : String(i + 1),
      catalogName: catalog?.name || null,
      catalogDuration: catalogMs != null ? formatDurationMs(catalogMs) : "—",
      uploadedName: uploaded ? uploaded.tag_title || uploaded.filename : null,
      uploadedDuration: uploadedMs != null ? formatDurationMs(uploadedMs) : "—",
      delta: deltaStr,
      deltaClass,
    });
  }
  return rows;
};

const formatDurationMs = (ms) => {
  const totalSec = Math.round(ms / 1000);
  const min = Math.floor(totalSec / 60);
  const sec = totalSec % 60;
  return `${min}:${sec.toString().padStart(2, "0")}`;
};

const fpScoreClass = (score) => {
  if (score == null) return "";
  if (score >= 95) return "scoreHigh";
  if (score >= 80) return "scoreMedium";
  return "scoreLow";
};

// Formatting
const formatStatus = (status) => {
  if (!status) return "";
  return status.toLowerCase().replace("_", " ");
};

const statusClass = (status) => {
  switch (status?.toUpperCase()) {
    case "COMPLETED":
      return "status-completed";
    case "PROCESSING":
      return "status-progress";
    case "ANALYZING":
      return "status-progress";
    case "IDENTIFYING_ALBUM":
      return "status-progress";
    case "MAPPING_TRACKS":
      return "status-progress";
    case "CONVERTING":
      return "status-converting";
    case "PENDING":
      return "status-pending";
    case "FAILED":
      return "status-failed";
    case "AWAITING_REVIEW":
      return "status-review";
    default:
      return "";
  }
};

const ticketClass = (ticketType) => {
  switch (ticketType?.toUpperCase()) {
    case "SUCCESS":
      return "ticket-success";
    case "REVIEW":
      return "ticket-review";
    case "FAILURE":
      return "ticket-failure";
    default:
      return "";
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
  min-width: 0;
  color: var(--text-base);
}
.pageHeader {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 28px;
}
.sectionTitle {
  margin: 0 0 10px;
  font-size: 32px;
  font-weight: 700;
  letter-spacing: -0.03em;
  line-height: 1.2;
}
.pageHeader p {
  margin: 0;
  color: var(--text-subdued);
  font-size: 14px;
  line-height: 1.5;
}
button {
  font: inherit;
  font-size: 13px;
  cursor: pointer;
}
button:disabled {
  opacity: 0.45;
  cursor: default;
}
button:focus-visible,
[tabindex]:focus-visible {
  outline: 2px solid #fff;
  outline-offset: 2px;
}
.refreshButton,
.folderButton,
.actionButton {
  min-height: 40px;
  padding: 8px 20px;
  border: 1px solid #727272;
  border-radius: 999px;
  background: transparent;
  color: var(--text-base);
  font-weight: 600;
}
.refreshButton:hover:not(:disabled),
.folderButton:hover,
.actionButton:hover:not(:disabled) {
  border-color: #fff;
  background: #ffffff0c;
}
.uploadSection {
  margin-bottom: 28px;
}
.uploadDropzone {
  padding: 32px 24px;
  border: 1px dashed #727272;
  background: #181818;
  border-radius: 8px;
  cursor: pointer;
}
.uploadDropzone:hover,
.uploadDropzone.dragging {
  border-color: #fff;
  background: #202020;
}
.dropzoneContent {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  text-align: center;
}
.dropzoneIcon {
  width: 32px;
  height: 32px;
  fill: none;
  stroke: var(--text-subdued);
  stroke-width: 1.5;
}
.dropzoneText {
  font-size: 16px;
  font-weight: 500;
}
.browseLink {
  text-decoration: underline;
}
.dropzoneHint {
  font-size: 12px;
  line-height: 1.5;
  color: var(--text-subdued);
}
.folderButton {
  margin-top: 4px;
}
.uploadProgress {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 12px;
  margin-top: 16px;
}
.progressBar {
  flex: 1 1 200px;
  height: 4px;
  border-radius: 999px;
  background: #535353;
  overflow: hidden;
}
.progressFill {
  height: 100%;
  background: var(--spotify-green);
}
.progressText {
  font-size: 13px;
  color: var(--text-subdued);
  overflow-wrap: anywhere;
}
.uploadError,
.uploadSuccess {
  margin-top: 16px;
  padding: 12px 16px;
  border-radius: 4px;
  font-size: 14px;
  line-height: 1.5;
  overflow-wrap: anywhere;
}
.uploadError {
  background: #f3727f12;
  color: #f3727f;
}
.uploadSuccess {
  background: #1ed76012;
  color: var(--spotify-green);
}
.statsSummary {
  display: flex;
  flex-wrap: wrap;
  gap: 12px 28px;
  padding-bottom: 24px;
  border-bottom: 1px solid var(--surface-border);
  margin-bottom: 24px;
  font-size: 14px;
  color: var(--text-subdued);
}
.statItem strong {
  color: var(--text-base);
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}
.statItem.warning strong {
  color: #f0bc65;
}
.statItem.danger strong {
  color: #f3727f;
}
.tabNav {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
  margin-bottom: 24px;
}
.tabButton {
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 36px;
  padding: 8px 16px;
  border: 0;
  border-radius: 999px;
  background: #242424;
  color: var(--text-base);
}
.tabButton:hover {
  background: #333;
}
.tabButton.active {
  background: #fff;
  color: #000;
}
.tabCount {
  font-size: 12px;
  opacity: 0.7;
}
.tabContent {
  min-height: 200px;
}
.emptyState {
  display: grid;
  place-items: center;
  min-height: 160px;
  padding: 24px;
  background: #181818;
  border-radius: 8px;
  color: var(--text-subdued);
  font-size: 14px;
}
.jobList,
.reviewList {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.jobItem,
.reviewItem {
  padding: 20px;
  background: #181818;
  border-radius: 8px;
  min-width: 0;
}
.jobHeader {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 14px;
}
.jobMain {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px 12px;
  min-width: 0;
  flex: 1 1 260px;
}
.jobFilename {
  font-size: 16px;
  font-weight: 500;
  overflow-wrap: anywhere;
}
.jobActions {
  display: flex;
  gap: 8px;
}
.actionButton {
  min-height: 36px;
  padding: 6px 16px;
}
.actionButton.primary {
  background: var(--spotify-green);
  border-color: transparent;
  color: #000;
  font-weight: 700;
}
.actionButton.primary:hover:not(:disabled) {
  background: var(--spotify-green-hover);
}
.actionButton.danger {
  display: grid;
  place-items: center;
  width: 36px;
  padding: 0;
  border: 0;
  color: var(--text-subdued);
}
.actionButton.danger:hover:not(:disabled) {
  color: #f3727f;
}
.actionButton.danger svg {
  width: 20px;
  height: 20px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.6;
}
.jobDetails,
.reviewMeta {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 20px;
  font-size: 12px;
}
.detailItem {
  display: inline-flex;
  flex-wrap: wrap;
  gap: 4px;
  min-width: 0;
  overflow-wrap: anywhere;
}
.detailLabel {
  color: var(--text-subdued);
}
.detailValue {
  color: var(--text-base);
}
.jobError {
  margin-top: 12px;
  color: #f3727f;
  font-size: 13px;
  line-height: 1.5;
  overflow-wrap: anywhere;
}
.statusBadge,
.ticketBadge,
.uploadTypeBadge {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  font-size: 12px;
  color: var(--text-subdued);
}
.statusBadge::before {
  content: "";
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: currentColor;
}
.ticketBadge,
.uploadTypeBadge {
  padding: 4px 8px;
  background: #2a2a2a;
  border-radius: 4px;
  text-transform: lowercase;
}
.statusBadge.status-progress,
.ticket-success {
  color: var(--spotify-green);
}
.statusBadge.status-review,
.statusBadge.status-converting,
.ticket-review {
  color: #f0bc65;
}
.statusBadge.status-failed,
.ticket-failure {
  color: #f3727f;
}
.reviewHeader {
  margin-bottom: 20px;
}
.reviewQuestion {
  font-size: 18px;
  font-weight: 700;
  line-height: 1.5;
  overflow-wrap: anywhere;
}
.reviewAlbumHeader {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 16px;
}
.reviewAlbumInfo {
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-width: 0;
}
.reviewAlbumArtist {
  font-size: 13px;
  color: var(--text-subdued);
}
.reviewAlbumName {
  font-size: 16px;
  font-weight: 600;
  overflow-wrap: anywhere;
}
.reviewAlbumScores {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 12px;
}
.scoreBadge,
.reviewTrackCount {
  font-size: 12px;
  color: var(--text-subdued);
  font-variant-numeric: tabular-nums;
}
.scoreHigh {
  color: var(--spotify-green);
}
.scoreMedium {
  color: #f0bc65;
}
.scoreLow {
  color: #f3727f;
}
.trackTableWrapper {
  max-width: 100%;
  overflow-x: auto;
  overscroll-behavior-x: contain;
  margin-bottom: 20px;
  border-radius: 4px;
}
.trackTable {
  width: 100%;
  min-width: 700px;
  border-collapse: collapse;
  font-size: 13px;
}
.trackTable th {
  padding: 12px;
  text-align: left;
  color: var(--text-subdued);
  font-weight: 500;
  background: #242424;
  border-bottom: 1px solid var(--surface-border);
}
.trackTable td {
  padding: 12px;
  border-bottom: 1px solid var(--surface-border);
}
.trackTable tbody tr:hover {
  background: #ffffff08;
}
.colNum {
  width: 32px;
  color: var(--text-subdued);
}
.colName {
  max-width: 220px;
  overflow-wrap: anywhere;
}
.colDur,
.colDelta {
  white-space: nowrap;
}
.mono {
  font-variant-numeric: tabular-nums;
}
.deltaOk {
  color: var(--spotify-green);
}
.deltaWarn {
  color: #f0bc65;
}
.deltaBad {
  color: #f3727f;
}
.reviewAlbumLoading {
  padding: 20px;
  color: var(--text-subdued);
  font-size: 13px;
}
.reviewOptions {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(min(240px, 100%), 1fr));
  gap: 12px;
  margin-bottom: 20px;
}
.reviewOption {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 8px;
  padding: 16px;
  text-align: left;
  border: 1px solid #727272;
  border-radius: 8px;
  background: transparent;
  color: var(--text-base);
}
.reviewOption:hover:not(:disabled) {
  border-color: #fff;
  background: #ffffff08;
}
.reviewOption.noMatch:hover:not(:disabled) {
  border-color: #f3727f;
}
.optionLabel {
  font-size: 14px;
  font-weight: 600;
  overflow-wrap: anywhere;
}
.optionDesc {
  font-size: 12px;
  line-height: 1.5;
  color: var(--text-subdued);
}
@media (max-width: 600px) {
  .jobItem,
  .reviewItem {
    padding: 16px;
  }
  .uploadDropzone {
    padding: 24px 16px;
  }
}
</style>
