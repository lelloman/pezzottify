<template>
  <div class="downloadManager">
    <h2 class="sectionTitle">Download Manager</h2>

    <!-- Downloader Status -->
    <div class="downloaderStatus" :class="downloaderStatusClass">
      <div class="statusMain">
        <span class="statusDot"></span>
        <span class="statusState">{{ downloaderState }}</span>
      </div>
      <div v-if="stats?.downloader?.uptime_secs" class="statusUptime">
        Uptime: {{ formatUptime(stats.downloader.uptime_secs) }}
      </div>
      <div v-if="stats?.downloader?.last_error" class="statusError">
        {{ stats.downloader.last_error }}
      </div>
    </div>

    <!-- Action Buttons -->
    <div class="actionButtons">
      <button class="actionButton" @click="openDownloadModal('album')">
        Download Album
      </button>
      <button class="actionButton" @click="openDownloadModal('artist')">
        Download Artist
      </button>
      <button class="refreshButton" @click="loadData" :disabled="isLoading">
        {{ isLoading ? "Loading..." : "Refresh" }}
      </button>
    </div>

    <!-- Stats Summary -->
    <div class="statsSummary">
      <span class="statItem">
        <strong>{{ stats?.queue?.pending ?? 0 }}</strong> pending
      </span>
      <span class="statItem">
        <strong>{{ stats?.queue?.in_progress ?? 0 }}</strong> in progress
      </span>
      <span class="statItem">
        <strong>{{ stats?.queue?.retry_waiting ?? 0 }}</strong> retrying
      </span>
      <span class="statItem success">
        <strong>{{ stats?.queue?.completed_today ?? 0 }}</strong> completed today
      </span>
      <span class="statItem danger">
        <strong>{{ stats?.queue?.failed_today ?? 0 }}</strong> failed today
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

    <div v-if="loadError" class="errorMessage">
      {{ loadError }}
    </div>

    <!-- Queue Tab -->
    <div v-if="activeTab === 'queue'" class="tabContent">
      <div v-if="queueItems.length === 0" class="emptyState">
        Queue is empty.
      </div>
      <div v-else class="queueList">
        <div v-for="item in queueItems" :key="item.id" class="queueItem" :class="statusClass(item.status)">
          <div class="queueItemHeader">
            <div class="queueItemMain">
              <span class="queueItemType">{{ formatContentType(item.content_type) }}</span>
              <span class="queueItemName">{{ formatItemName(item) }}</span>
            </div>
            <div class="queueItemActions">
              <span class="statusBadge" :class="statusClass(item.status)">
                {{ formatStatus(item.status) }}
              </span>
              <button
                v-if="item.status === 'FAILED'"
                class="retryButton"
                @click="handleRetry(item.id, false)"
                :disabled="retryingItems[item.id]"
              >
                {{ retryingItems[item.id] ? "..." : "Retry" }}
              </button>
              <button
                v-if="item.status === 'IN_PROGRESS' || item.status === 'RETRY_WAITING'"
                class="forceRetryButton"
                @click="handleRetry(item.id, true)"
                :disabled="retryingItems[item.id]"
              >
                {{ retryingItems[item.id] ? "..." : "Force" }}
              </button>
              <button
                class="deleteButton"
                @click="confirmDelete(item)"
                :disabled="deletingItems[item.id]"
              >
                {{ deletingItems[item.id] ? "..." : "Delete" }}
              </button>
            </div>
          </div>
          <!-- Progress bar for requests with children -->
          <div v-if="item.progress && item.progress.total_children > 0" class="progressSection">
            <div class="progressBar">
              <div
                class="progressFill"
                :style="{ width: getProgressPercent(item.progress) + '%' }"
                :class="{ 'has-failed': item.progress.failed > 0 }"
              ></div>
            </div>
            <span class="progressText">
              {{ item.progress.completed }}/{{ item.progress.total_children }} completed
              <span v-if="item.progress.failed > 0" class="progressFailed">
                ({{ item.progress.failed }} failed)
              </span>
              <span v-if="item.progress.in_progress > 0" class="progressActive">
                ({{ item.progress.in_progress }} active)
              </span>
            </span>
          </div>
          <div class="queueItemDetails">
            <span class="detailItem">
              <span class="detailLabel">Priority:</span>
              <span class="detailValue">{{ formatPriority(item.priority) }}</span>
            </span>
            <span class="detailItem">
              <span class="detailLabel">Created:</span>
              <span class="detailValue">{{ formatDate(item.created_at) }}</span>
            </span>
            <span v-if="item.last_attempt_at" class="detailItem">
              <span class="detailLabel">Last attempt:</span>
              <span class="detailValue">{{ formatDate(item.last_attempt_at) }}</span>
            </span>
            <span v-if="item.next_retry_at" class="detailItem">
              <span class="detailLabel">Next retry:</span>
              <span class="detailValue">{{ formatDate(item.next_retry_at) }}</span>
            </span>
            <span v-if="item.retry_count > 0" class="detailItem">
              <span class="detailLabel">Retries:</span>
              <span class="detailValue">{{ item.retry_count }} / {{ item.max_retries }}</span>
            </span>
          </div>
          <div v-if="item.error_type || item.error_message" class="queueItemError">
            <span v-if="item.error_type" class="errorType">{{ item.error_type }}</span>
            <span v-if="item.error_message">{{ item.error_message }}</span>
          </div>
        </div>
      </div>
    </div>

    <!-- Failed Tab -->
    <div v-if="activeTab === 'failed'" class="tabContent">
      <div v-if="failedItems.length === 0" class="emptyState">
        No failed downloads.
      </div>
      <div v-else class="queueList">
        <div v-for="item in failedItems" :key="item.id" class="queueItem status-failed">
          <div class="queueItemHeader">
            <div class="queueItemMain">
              <span class="queueItemType">{{ formatContentType(item.content_type) }}</span>
              <span class="queueItemName">{{ formatItemName(item) }}</span>
            </div>
            <div class="queueItemActions">
              <button
                class="retryButton"
                @click="handleRetry(item.id, false)"
                :disabled="retryingItems[item.id]"
              >
                {{ retryingItems[item.id] ? "..." : "Retry" }}
              </button>
              <button
                class="deleteButton"
                @click="confirmDelete(item)"
                :disabled="deletingItems[item.id]"
              >
                {{ deletingItems[item.id] ? "..." : "Delete" }}
              </button>
            </div>
          </div>
          <!-- Progress info for failed requests with children -->
          <div v-if="item.progress && item.progress.total_children > 0" class="progressSection">
            <span class="progressText">
              {{ item.progress.completed }}/{{ item.progress.total_children }} completed,
              {{ item.progress.failed }} failed
            </span>
          </div>
          <div class="queueItemDetails">
            <span class="detailItem">
              <span class="detailLabel">Created:</span>
              <span class="detailValue">{{ formatDate(item.created_at) }}</span>
            </span>
            <span v-if="item.last_attempt_at" class="detailItem">
              <span class="detailLabel">Last attempt:</span>
              <span class="detailValue">{{ formatDate(item.last_attempt_at) }}</span>
            </span>
            <span class="detailItem">
              <span class="detailLabel">Retries:</span>
              <span class="detailValue">{{ item.retry_count }} / {{ item.max_retries }}</span>
            </span>
          </div>
          <div v-if="item.error_type || item.error_message" class="queueItemError">
            <span v-if="item.error_type" class="errorType">{{ item.error_type }}</span>
            <span v-if="item.error_message">{{ item.error_message }}</span>
          </div>
        </div>
      </div>
    </div>

    <!-- Downloaded Tab -->
    <div v-if="activeTab === 'downloaded'" class="tabContent">
      <div v-if="completedItems.length === 0" class="emptyState">
        No completed downloads yet.
      </div>
      <div v-else class="queueList">
        <div v-for="item in completedItems" :key="item.id" class="queueItem completed">
          <div class="queueItemMain">
            <span class="queueItemType">{{ formatContentType(item.content_type) }}</span>
            <span class="queueItemName">{{ formatItemName(item) }}</span>
          </div>
          <div class="queueItemMeta">
            <span class="statusBadge status-completed">completed</span>
            <span class="queueItemTime">{{ formatDate(item.completed_at || item.updated_at) }}</span>
          </div>
        </div>
      </div>
    </div>

    <!-- Audit Log Tab -->
    <div v-if="activeTab === 'audit'" class="tabContent">
      <div v-if="auditLog.length === 0" class="emptyState">
        No audit log entries.
      </div>
      <table v-else class="auditTable">
        <thead>
          <tr>
            <th class="colTime">Time</th>
            <th class="colEvent">Event</th>
            <th class="colUser">User</th>
            <th class="colDetails">Details</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="entry in auditLog" :key="entry.id" class="auditRow">
            <td class="colTime">{{ formatDate(entry.timestamp) }}</td>
            <td class="colEvent">
              <span class="eventBadge" :class="eventClass(entry.event_type)">
                {{ formatEventType(entry.event_type) }}
              </span>
            </td>
            <td class="colUser">
              <span v-if="entry.user_id" class="auditUser">{{ entry.user_id }}</span>
              <span v-else class="textMuted">—</span>
            </td>
            <td class="colDetails">{{ formatAuditDetails(entry) }}</td>
          </tr>
        </tbody>
      </table>
    </div>

    <!-- Statistics Tab -->
    <div v-if="activeTab === 'statistics'" class="tabContent">
      <!-- Period Selector -->
      <div class="periodSelector">
        <button
          v-for="p in periods"
          :key="p.id"
          class="periodButton"
          :class="{ active: selectedPeriod === p.id }"
          @click="selectPeriod(p.id)"
        >
          {{ p.label }}
        </button>
      </div>

      <!-- Totals Summary Cards -->
      <div v-if="statsHistory" class="statsTotals">
        <div class="totalCard">
          <span class="totalValue">{{ statsHistory.total_albums }}</span>
          <span class="totalLabel">Albums</span>
        </div>
        <div class="totalCard">
          <span class="totalValue">{{ statsHistory.total_tracks }}</span>
          <span class="totalLabel">Tracks</span>
        </div>
        <div class="totalCard">
          <span class="totalValue">{{ statsHistory.total_images }}</span>
          <span class="totalLabel">Images</span>
        </div>
        <div class="totalCard">
          <span class="totalValue">{{ formatBytes(statsHistory.total_bytes) }}</span>
          <span class="totalLabel">Downloaded</span>
        </div>
        <div class="totalCard totalFailures">
          <span class="totalValue">{{ statsHistory.total_failures }}</span>
          <span class="totalLabel">Failures</span>
        </div>
      </div>

      <!-- Downloads Chart -->
      <div v-if="statsHistory" class="chartSection">
        <h4 class="chartTitle">Downloads Over Time</h4>
        <div class="chartContainer">
          <Line
            v-if="downloadsChartData"
            :data="downloadsChartData"
            :options="lineChartOptions"
          />
          <div v-else class="noData">No data available for this period.</div>
        </div>
      </div>

      <!-- Data Table -->
      <div v-if="statsHistory?.entries?.length > 0" class="tableSection">
        <h4 class="chartTitle">Period Breakdown</h4>
        <div class="tableWrapper">
          <table class="dataTable">
            <thead>
              <tr>
                <th>Period</th>
                <th>Albums</th>
                <th>Tracks</th>
                <th>Images</th>
                <th>Size</th>
                <th>Failures</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="entry in statsHistory.entries" :key="entry.period_start">
                <td>{{ formatPeriodDate(entry.period_start) }}</td>
                <td>{{ entry.albums }}</td>
                <td>{{ entry.tracks }}</td>
                <td>{{ entry.images }}</td>
                <td>{{ formatBytes(entry.bytes) }}</td>
                <td :class="{ 'text-danger': entry.failures > 0 }">{{ entry.failures }}</td>
              </tr>
            </tbody>
          </table>
        </div>
      </div>

      <div v-if="!statsHistory && !isLoadingStats" class="emptyState">
        No statistics data available.
      </div>
      <div v-if="isLoadingStats" class="emptyState">
        Loading statistics...
      </div>
    </div>

    <!-- Download Request Modal -->
    <div v-if="showDownloadModal" class="detailOverlay" @click.self="closeDownloadModal">
      <div class="detailPanel downloadModal">
        <div class="detailHeader">
          <h3 class="detailTitle">
            {{ downloadModalType === "album" ? "Download Album" : "Download Artist Discography" }}
          </h3>
          <button class="closeDetailButton" @click="closeDownloadModal">×</button>
        </div>
        <div class="modalContent">
          <div class="formGroup">
            <label class="formLabel">
              {{ downloadModalType === "album" ? "Album ID" : "Artist ID" }}
            </label>
            <input
              v-model="downloadForm.id"
              type="text"
              class="formInput"
              :placeholder="downloadModalType === 'album' ? 'External album ID' : 'External artist ID'"
            />
          </div>
          <div v-if="downloadModalType === 'album'" class="formGroup">
            <label class="formLabel">Album Name</label>
            <input
              v-model="downloadForm.albumName"
              type="text"
              class="formInput"
              placeholder="Album name (for display)"
            />
          </div>
          <div class="formGroup">
            <label class="formLabel">Artist Name</label>
            <input
              v-model="downloadForm.artistName"
              type="text"
              class="formInput"
              placeholder="Artist name (for display)"
            />
          </div>
          <div v-if="downloadError" class="modalError">
            {{ downloadError }}
          </div>
          <div v-if="downloadSuccess" class="modalSuccess">
            {{ downloadSuccess }}
          </div>
          <div class="modalActions">
            <button class="cancelButton" @click="closeDownloadModal">Cancel</button>
            <button
              class="confirmButton"
              @click="submitDownloadRequest"
              :disabled="isSubmitting || !isFormValid"
            >
              {{ isSubmitting ? "Submitting..." : "Download" }}
            </button>
          </div>
        </div>
      </div>
    </div>

    <!-- Delete Confirmation Modal -->
    <div v-if="showDeleteModal" class="detailOverlay" @click.self="closeDeleteModal">
      <div class="detailPanel deleteModal">
        <div class="detailHeader">
          <h3 class="detailTitle">Delete Download Request</h3>
          <button class="closeDetailButton" @click="closeDeleteModal">×</button>
        </div>
        <div class="modalContent">
          <p class="deleteWarning">
            Are you sure you want to delete this download request?
          </p>
          <div class="deleteItemInfo">
            <span class="queueItemType">{{ formatContentType(itemToDelete?.content_type) }}</span>
            <span class="queueItemName">{{ formatItemName(itemToDelete) }}</span>
          </div>
          <div v-if="deleteError" class="modalError">
            {{ deleteError }}
          </div>
          <div class="modalActions">
            <button class="cancelButton" @click="closeDeleteModal">Cancel</button>
            <button
              class="deleteConfirmButton"
              @click="executeDelete"
              :disabled="isDeleting"
            >
              {{ isDeleting ? "Deleting..." : "Delete" }}
            </button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, reactive, computed, onMounted, onUnmounted, watch } from "vue";
import { Line } from "vue-chartjs";
import {
  Chart as ChartJS,
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  Title,
  Tooltip,
  Legend,
  Filler,
} from "chart.js";
import { useRemoteStore } from "@/store/remote";

// Register Chart.js components
ChartJS.register(
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  Title,
  Tooltip,
  Legend,
  Filler,
);

const remoteStore = useRemoteStore();

// Download modal state
const showDownloadModal = ref(false);
const downloadModalType = ref(null);
const downloadForm = reactive({
  id: "",
  albumName: "",
  artistName: "",
});
const isSubmitting = ref(false);
const downloadError = ref(null);
const downloadSuccess = ref(null);

const isFormValid = computed(() => {
  if (!downloadForm.id || !downloadForm.artistName) return false;
  if (downloadModalType.value === "album" && !downloadForm.albumName) return false;
  return true;
});

const downloaderState = computed(() => {
  if (stats.value === null) return "Checking...";
  if (!stats.value.downloader) return "Unknown";
  return stats.value.downloader.state || "Unknown";
});

const downloaderStatusClass = computed(() => {
  const state = stats.value?.downloader?.state;
  if (!state) return "status-unknown";
  if (state === "Healthy") return "status-online";
  if (state === "LoggingIn" || state === "Booting") return "status-pending";
  return "status-offline";
});

const openDownloadModal = (type) => {
  downloadModalType.value = type;
  downloadForm.id = "";
  downloadForm.albumName = "";
  downloadForm.artistName = "";
  downloadError.value = null;
  downloadSuccess.value = null;
  showDownloadModal.value = true;
};

const closeDownloadModal = () => {
  showDownloadModal.value = false;
  downloadModalType.value = null;
};

const submitDownloadRequest = async () => {
  isSubmitting.value = true;
  downloadError.value = null;
  downloadSuccess.value = null;

  let result;
  if (downloadModalType.value === "album") {
    result = await remoteStore.requestAlbumDownload(
      downloadForm.id,
      downloadForm.albumName,
      downloadForm.artistName,
    );
  } else {
    result = await remoteStore.requestDiscographyDownload(
      downloadForm.id,
      downloadForm.artistName,
    );
  }

  isSubmitting.value = false;

  if (result.error) {
    downloadError.value = typeof result.error === "string" ? result.error : JSON.stringify(result.error);
  } else {
    downloadSuccess.value = downloadModalType.value === "album"
      ? "Album queued for download!"
      : "Discography queued for download!";
    await loadData();
    setTimeout(() => {
      closeDownloadModal();
    }, 1500);
  }
};

// Tab and data state
const activeTab = ref("queue");
const isLoading = ref(false);
const loadError = ref(null);

const stats = ref(null);
const queueItems = ref([]);
const failedItems = ref([]);
const completedItems = ref([]);
const auditLog = ref([]);
const retryingItems = reactive({});
const deletingItems = reactive({});

// Delete modal state
const showDeleteModal = ref(false);
const itemToDelete = ref(null);
const isDeleting = ref(false);
const deleteError = ref(null);

// Statistics state
const selectedPeriod = ref("daily");
const statsHistory = ref(null);
const isLoadingStats = ref(false);

const periods = [
  { id: "hourly", label: "Last 48 Hours" },
  { id: "daily", label: "Last 30 Days" },
  { id: "weekly", label: "Last 12 Weeks" },
];

const loadStatsHistory = async () => {
  isLoadingStats.value = true;
  const result = await remoteStore.fetchDownloadStatsHistory(selectedPeriod.value);
  statsHistory.value = result;
  isLoadingStats.value = false;
};

const selectPeriod = async (period) => {
  selectedPeriod.value = period;
  await loadStatsHistory();
};

// Watch for tab change to load statistics when needed
watch(
  () => activeTab.value,
  async (newTab) => {
    if (newTab === "statistics" && !statsHistory.value) {
      await loadStatsHistory();
    }
  },
);

// Chart configuration
const formatPeriodDate = (timestamp) => {
  const date = new Date(timestamp * 1000);
  if (selectedPeriod.value === "hourly") {
    return date.toLocaleString(undefined, {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  } else if (selectedPeriod.value === "weekly") {
    return `Week of ${date.toLocaleDateString(undefined, { month: "short", day: "numeric" })}`;
  }
  return date.toLocaleDateString(undefined, { month: "short", day: "numeric" });
};

const downloadsChartData = computed(() => {
  if (!statsHistory.value?.entries?.length) return null;

  const entries = statsHistory.value.entries;
  const labels = entries.map((e) => formatPeriodDate(e.period_start));

  return {
    labels,
    datasets: [
      {
        label: "Albums",
        data: entries.map((e) => e.albums),
        borderColor: "#1db954",
        backgroundColor: "rgba(29, 185, 84, 0.1)",
        fill: true,
        tension: 0.3,
      },
      {
        label: "Tracks",
        data: entries.map((e) => e.tracks),
        borderColor: "#3b82f6",
        backgroundColor: "rgba(59, 130, 246, 0.1)",
        fill: true,
        tension: 0.3,
      },
      {
        label: "Failures",
        data: entries.map((e) => e.failures),
        borderColor: "#dc2626",
        backgroundColor: "rgba(220, 38, 38, 0.1)",
        fill: true,
        tension: 0.3,
      },
    ],
  };
});

const lineChartOptions = {
  responsive: true,
  maintainAspectRatio: false,
  interaction: {
    mode: "index",
    intersect: false,
  },
  plugins: {
    legend: {
      position: "top",
      labels: {
        color: "#a1a1aa",
        usePointStyle: true,
        padding: 16,
      },
    },
    tooltip: {
      backgroundColor: "#27272a",
      titleColor: "#fafafa",
      bodyColor: "#a1a1aa",
      borderColor: "#3f3f46",
      borderWidth: 1,
      padding: 12,
    },
  },
  scales: {
    x: {
      grid: {
        color: "rgba(63, 63, 70, 0.3)",
      },
      ticks: {
        color: "#a1a1aa",
        maxRotation: 45,
        minRotation: 45,
      },
    },
    y: {
      grid: {
        color: "rgba(63, 63, 70, 0.3)",
      },
      ticks: {
        color: "#a1a1aa",
        precision: 0,
      },
      beginAtZero: true,
    },
  },
};

const tabs = computed(() => [
  { id: "queue", label: "Queue", count: queueItems.value.length },
  { id: "failed", label: "Failed", count: failedItems.value.length },
  { id: "downloaded", label: "Downloaded", count: completedItems.value.length },
  { id: "audit", label: "Audit Log" },
  { id: "statistics", label: "Statistics" },
]);

const loadData = async () => {
  isLoading.value = true;
  loadError.value = null;

  try {
    const [statsResult, queueResult, failedResult, completedResult, auditResult] = await Promise.all([
      remoteStore.fetchDownloadStats(),
      remoteStore.fetchDownloadQueue(),
      remoteStore.fetchFailedDownloads(100, 0),
      remoteStore.fetchDownloadCompleted(100, 0),
      remoteStore.fetchDownloadAuditLog(100, 0),
    ]);

    stats.value = statsResult;
    queueItems.value = queueResult?.items || [];
    failedItems.value = failedResult?.items || [];
    completedItems.value = completedResult?.items || [];
    auditLog.value = auditResult?.entries || [];

    if (!statsResult) {
      loadError.value = "Download manager may not be enabled on this server.";
    }
  } catch {
    loadError.value = "Failed to load download manager data.";
  }

  isLoading.value = false;
};

const handleRetry = async (itemId, force = false) => {
  retryingItems[itemId] = true;

  const result = await remoteStore.retryDownload(itemId, force);

  if (result.error) {
    alert(result.error);
  } else {
    await loadData();
  }

  retryingItems[itemId] = false;
};

const confirmDelete = (item) => {
  itemToDelete.value = item;
  deleteError.value = null;
  showDeleteModal.value = true;
};

const closeDeleteModal = () => {
  showDeleteModal.value = false;
  itemToDelete.value = null;
  deleteError.value = null;
};

const executeDelete = async () => {
  if (!itemToDelete.value) return;

  const itemId = itemToDelete.value.id;
  isDeleting.value = true;
  deletingItems[itemId] = true;
  deleteError.value = null;

  const result = await remoteStore.deleteDownloadRequest(itemId);

  if (result.error) {
    deleteError.value = result.error;
    isDeleting.value = false;
    deletingItems[itemId] = false;
  } else {
    closeDeleteModal();
    await loadData();
    deletingItems[itemId] = false;
    isDeleting.value = false;
  }
};

const formatItemName = (item) => {
  const name = item.content_name || item.content_id;
  if (item.artist_name) {
    return `${name} - ${item.artist_name}`;
  }
  return name;
};

const formatPriority = (priority) => {
  if (!priority) return "normal";
  const p = priority.toLowerCase();
  if (p === "high" || p === "system") return "high";
  if (p === "low") return "low";
  return "normal";
};

const formatUptime = (secs) => {
  if (secs < 60) return `${secs}s`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m`;
  if (secs < 86400) return `${Math.floor(secs / 3600)}h ${Math.floor((secs % 3600) / 60)}m`;
  return `${Math.floor(secs / 86400)}d ${Math.floor((secs % 86400) / 3600)}h`;
};

const formatDate = (timestamp) => {
  if (!timestamp) return "—";
  const date = new Date(timestamp * 1000);
  return date.toLocaleString();
};

const formatContentType = (type) => {
  if (!type) return "";
  return type.replace("_", " ").toLowerCase();
};

const formatStatus = (status) => {
  if (!status) return "";
  return status.toLowerCase().replace("_", " ");
};

const statusClass = (status) => {
  switch (status?.toUpperCase()) {
    case "COMPLETED":
      return "status-completed";
    case "IN_PROGRESS":
      return "status-progress";
    case "PENDING":
      return "status-pending";
    case "FAILED":
      return "status-failed";
    case "RETRY_WAITING":
      return "status-retry";
    default:
      return "";
  }
};

const eventClass = (eventType) => {
  if (eventType?.includes("completed") || eventType?.includes("success")) {
    return "event-success";
  }
  if (eventType?.includes("failed") || eventType?.includes("error")) {
    return "event-error";
  }
  if (eventType?.includes("retry")) {
    return "event-retry";
  }
  return "event-info";
};

const formatEventType = (eventType) => {
  if (!eventType) return "—";
  return eventType
    .split("_")
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(" ");
};

const getProgressPercent = (progress) => {
  if (!progress || progress.total_children === 0) return 0;
  const terminal = progress.completed + progress.failed;
  return Math.round((terminal / progress.total_children) * 100);
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
  if (ms == null) return "—";
  if (ms < 1000) return `${ms}ms`;
  const secs = ms / 1000;
  if (secs < 60) return `${secs.toFixed(1)}s`;
  const mins = Math.floor(secs / 60);
  const remainingSecs = Math.floor(secs % 60);
  return `${mins}m ${remainingSecs}s`;
};

const formatAuditDetails = (entry) => {
  const details = entry.details;
  const contentName = entry.content_id || "";
  const eventType = entry.event_type;

  // Build context prefix from content info
  let prefix = "";
  if (entry.content_type && contentName) {
    const type = entry.content_type.replace("_", " ").toLowerCase();
    prefix = `${type} "${contentName}" — `;
  }

  if (!details) {
    return prefix || "—";
  }

  // Parse details based on event type
  switch (eventType) {
    case "REQUEST_CREATED": {
      const name = details.content_name || contentName;
      const artist = details.artist_name ? ` by ${details.artist_name}` : "";
      const pos = details.queue_position != null ? `, queue #${details.queue_position}` : "";
      return `${name}${artist}${pos}`;
    }

    case "CHILDREN_CREATED": {
      const tracks = details.track_count || 0;
      const images = details.image_count || 0;
      const parts = [];
      if (tracks > 0) parts.push(`${tracks} track${tracks !== 1 ? "s" : ""}`);
      if (images > 0) parts.push(`${images} image${images !== 1 ? "s" : ""}`);
      return `${prefix}spawned ${parts.join(", ") || "no children"}`;
    }

    case "DOWNLOAD_COMPLETED": {
      const size = formatBytes(details.bytes_downloaded);
      const duration = formatDuration(details.duration_ms);
      const tracks = details.tracks_downloaded;
      let result = `${prefix}${size} in ${duration}`;
      if (tracks != null) {
        result += ` (${tracks} track${tracks !== 1 ? "s" : ""})`;
      }
      return result;
    }

    case "DOWNLOAD_FAILED": {
      const errType = details.error_type || "unknown";
      const errMsg = details.error_message || "";
      const retries = details.retry_count || 0;
      return `${prefix}${errType}: ${errMsg} (after ${retries} retries)`;
    }

    case "RETRY_SCHEDULED": {
      const errType = details.error_type || "error";
      const backoff = details.backoff_secs ? `${details.backoff_secs}s` : "?";
      const attempt = details.retry_count || 0;
      return `${prefix}${errType}, retry #${attempt + 1} in ${backoff}`;
    }

    case "ADMIN_RETRY": {
      const prevErr = details.previous_error_type || "unknown";
      return `${prefix}reset from ${prevErr} error`;
    }

    case "WATCHDOG_QUEUED": {
      const reason = details.reason || "missing content";
      return `${prefix}${reason}`;
    }

    case "WATCHDOG_SCAN_STARTED":
      return "Integrity scan started";

    case "WATCHDOG_SCAN_COMPLETED": {
      const total = details.total_missing || 0;
      const queued = details.items_queued || 0;
      const skipped = details.items_skipped || 0;
      const duration = formatDuration(details.scan_duration_ms);
      if (total === 0) {
        return `No issues found (${duration})`;
      }
      return `Found ${total} missing, queued ${queued}, skipped ${skipped} (${duration})`;
    }

    case "DOWNLOAD_STARTED":
      return prefix || "Processing started";

    default:
      // Fallback: show raw details as key-value pairs
      return prefix + Object.entries(details)
        .map(([k, v]) => `${k}: ${v}`)
        .join(", ");
  }
};

// Auto-refresh every 10 seconds
const REFRESH_INTERVAL = 10000;
let refreshInterval = null;

onMounted(() => {
  loadData();
  refreshInterval = setInterval(loadData, REFRESH_INTERVAL);
});

onUnmounted(() => {
  if (refreshInterval) {
    clearInterval(refreshInterval);
    refreshInterval = null;
  }
});
</script>

<style scoped>
.downloadManager {
  width: 100%;
}

.sectionTitle {
  font-size: var(--text-2xl);
  font-weight: var(--font-bold);
  color: var(--text-base);
  margin: 0 0 var(--spacing-4) 0;
}

/* Downloader Status */
.downloaderStatus {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--spacing-4);
  padding: var(--spacing-3) var(--spacing-4);
  border-radius: var(--radius-lg);
  margin-bottom: var(--spacing-4);
  font-size: var(--text-sm);
}

.downloaderStatus.status-online {
  background-color: rgba(34, 197, 94, 0.15);
}

.downloaderStatus.status-offline {
  background-color: rgba(220, 38, 38, 0.15);
}

.downloaderStatus.status-pending {
  background-color: rgba(249, 115, 22, 0.15);
}

.downloaderStatus.status-unknown {
  background-color: rgba(156, 163, 175, 0.15);
}

.statusMain {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  font-weight: var(--font-semibold);
}

.statusDot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
}

.status-online .statusDot { background-color: #22c55e; }
.status-online .statusState { color: #22c55e; }
.status-offline .statusDot { background-color: #dc2626; }
.status-offline .statusState { color: #dc2626; }
.status-pending .statusDot { background-color: #f97316; }
.status-pending .statusState { color: #f97316; }
.status-unknown .statusDot { background-color: #9ca3af; }
.status-unknown .statusState { color: #9ca3af; }

.statusUptime {
  color: var(--text-subdued);
}

.statusError {
  color: #dc2626;
  font-size: var(--text-xs);
  flex-basis: 100%;
}

/* Action Buttons */
.actionButtons {
  display: flex;
  gap: var(--spacing-3);
  margin-bottom: var(--spacing-4);
}

.actionButton {
  padding: var(--spacing-2) var(--spacing-4);
  background-color: var(--spotify-green);
  color: white;
  border: none;
  border-radius: var(--radius-md);
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  cursor: pointer;
  transition: background-color var(--transition-fast);
}

.actionButton:hover {
  background-color: #1ed760;
}

.refreshButton {
  margin-left: auto;
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

.statItem.success strong {
  color: #22c55e;
}

.statItem.danger strong {
  color: #dc2626;
}

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

/* Error Message */
.errorMessage {
  padding: var(--spacing-3) var(--spacing-4);
  background-color: rgba(220, 38, 38, 0.1);
  border: 1px solid #dc2626;
  border-radius: var(--radius-md);
  color: #dc2626;
  font-size: var(--text-sm);
  margin-bottom: var(--spacing-4);
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
  font-size: var(--text-base);
}

/* Queue List */
.queueList {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
}

.queueItem {
  background-color: var(--bg-elevated-base);
  border-radius: var(--radius-md);
  padding: var(--spacing-3) var(--spacing-4);
  border-left: 3px solid var(--border-subdued);
}

.queueItem.status-pending { border-left-color: #9ca3af; }
.queueItem.status-progress { border-left-color: #3b82f6; }
.queueItem.status-completed { border-left-color: #22c55e; }
.queueItem.status-failed { border-left-color: #dc2626; }
.queueItem.status-retry { border-left-color: #f97316; }

.queueItemMain {
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
  margin-bottom: var(--spacing-2);
}

.queueItemType {
  font-size: var(--text-xs);
  text-transform: uppercase;
  color: var(--text-subdued);
  background-color: var(--bg-highlight);
  padding: 2px 6px;
  border-radius: var(--radius-sm);
}

.queueItemName {
  font-weight: var(--font-medium);
  color: var(--text-base);
}

.queueItemArtist {
  color: var(--text-subdued);
  font-size: var(--text-sm);
}

.queueItemMeta {
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
  font-size: var(--text-sm);
}

.queueItemTime {
  color: var(--text-subdued);
  font-size: var(--text-xs);
}

.queueItemError {
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

.status-completed {
  background-color: rgba(34, 197, 94, 0.15);
  color: #22c55e;
}

.status-progress {
  background-color: rgba(59, 130, 246, 0.15);
  color: #3b82f6;
}

.status-pending {
  background-color: rgba(156, 163, 175, 0.15);
  color: #9ca3af;
}

.status-failed {
  background-color: rgba(220, 38, 38, 0.15);
  color: #dc2626;
}

.status-retry {
  background-color: rgba(249, 115, 22, 0.15);
  color: #f97316;
}

/* Progress Bar */
.progressSection {
  margin: var(--spacing-2) 0;
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
}

.progressBar {
  flex: 1;
  height: 6px;
  background-color: var(--bg-highlight);
  border-radius: 3px;
  overflow: hidden;
  max-width: 200px;
}

.progressFill {
  height: 100%;
  background-color: var(--spotify-green);
  border-radius: 3px;
  transition: width 0.3s ease;
}

.progressFill.has-failed {
  background-color: #f97316;
}

.progressText {
  font-size: var(--text-xs);
  color: var(--text-subdued);
}

.progressFailed {
  color: #dc2626;
}

.progressActive {
  color: #3b82f6;
}

/* Retry Button */
.retryButton {
  padding: 2px 10px;
  background-color: var(--spotify-green);
  color: white;
  border: none;
  border-radius: var(--radius-md);
  font-size: var(--text-xs);
  cursor: pointer;
  transition: background-color var(--transition-fast);
}

.retryButton:hover:not(:disabled) {
  background-color: #1ed760;
}

.retryButton:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.forceRetryButton {
  padding: 2px 10px;
  background-color: #f97316;
  color: white;
  border: none;
  border-radius: var(--radius-md);
  font-size: var(--text-xs);
  cursor: pointer;
  transition: background-color var(--transition-fast);
}

.forceRetryButton:hover:not(:disabled) {
  background-color: #ea580c;
}

.forceRetryButton:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.deleteButton {
  padding: 2px 10px;
  background-color: transparent;
  color: #dc2626;
  border: 1px solid #dc2626;
  border-radius: var(--radius-md);
  font-size: var(--text-xs);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.deleteButton:hover:not(:disabled) {
  background-color: #dc2626;
  color: white;
}

.deleteButton:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

/* Queue Item Layout */
.queueItemHeader {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: var(--spacing-3);
}

.queueItemActions {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  flex-shrink: 0;
}

.queueItemDetails {
  display: flex;
  flex-wrap: wrap;
  gap: var(--spacing-3);
  margin-top: var(--spacing-2);
  font-size: var(--text-xs);
}

.detailItem {
  display: inline-flex;
  align-items: center;
  gap: var(--spacing-1);
}

.detailLabel {
  color: var(--text-subdued);
}

.detailValue {
  color: var(--text-base);
}

.queueItemError .errorType {
  display: inline-block;
  padding: 1px 6px;
  background-color: rgba(220, 38, 38, 0.15);
  border-radius: var(--radius-sm);
  margin-right: var(--spacing-2);
  font-weight: var(--font-medium);
}

/* Delete Modal */
.deleteWarning {
  color: var(--text-subdued);
  margin: 0 0 var(--spacing-4) 0;
}

.deleteItemInfo {
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
  padding: var(--spacing-3);
  background-color: var(--bg-base);
  border-radius: var(--radius-md);
  margin-bottom: var(--spacing-4);
}

.deleteConfirmButton {
  padding: var(--spacing-2) var(--spacing-4);
  background-color: #dc2626;
  border: none;
  border-radius: var(--radius-md);
  color: white;
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  cursor: pointer;
}

.deleteConfirmButton:hover:not(:disabled) {
  background-color: #b91c1c;
}

.deleteConfirmButton:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

/* Audit Table */
.auditTable {
  width: 100%;
  border-collapse: collapse;
  font-size: var(--text-sm);
}

.auditTable thead {
  position: sticky;
  top: 0;
  background-color: var(--bg-base);
}

.auditTable th {
  text-align: left;
  padding: var(--spacing-2) var(--spacing-3);
  color: var(--text-subdued);
  font-weight: var(--font-medium);
  font-size: var(--text-xs);
  text-transform: uppercase;
  border-bottom: 1px solid var(--border-subdued);
}

.auditTable td {
  padding: var(--spacing-2) var(--spacing-3);
  border-bottom: 1px solid var(--border-subdued);
  vertical-align: top;
}

.auditRow:hover {
  background-color: var(--bg-highlight);
}

.colTime {
  width: 140px;
  white-space: nowrap;
  color: var(--text-subdued);
  font-size: var(--text-xs);
}

.colEvent {
  width: 160px;
}

.colUser {
  width: 120px;
}

.colDetails {
  color: var(--text-base);
}

.auditUser {
  color: var(--spotify-green);
  font-size: var(--text-xs);
}

.textMuted {
  color: var(--text-subdued);
}

/* Event Badge */
.eventBadge {
  display: inline-block;
  padding: 2px 8px;
  border-radius: var(--radius-md);
  font-size: var(--text-xs);
  font-weight: var(--font-medium);
}

.event-success {
  background-color: rgba(34, 197, 94, 0.15);
  color: #22c55e;
}

.event-error {
  background-color: rgba(220, 38, 38, 0.15);
  color: #dc2626;
}

.event-retry {
  background-color: rgba(249, 115, 22, 0.15);
  color: #f97316;
}

.event-info {
  background-color: rgba(59, 130, 246, 0.15);
  color: #3b82f6;
}

/* Modal */
.detailOverlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: rgba(0, 0, 0, 0.7);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
  padding: var(--spacing-4);
}

.detailPanel {
  background-color: var(--bg-elevated-base);
  border-radius: var(--radius-lg);
  max-width: 450px;
  width: 100%;
  overflow: hidden;
}

.detailHeader {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--spacing-4);
  border-bottom: 1px solid var(--border-subdued);
}

.detailTitle {
  font-size: var(--text-lg);
  font-weight: var(--font-bold);
  color: var(--text-base);
  margin: 0;
}

.closeDetailButton {
  background: none;
  border: none;
  color: var(--text-subdued);
  font-size: var(--text-2xl);
  cursor: pointer;
  padding: var(--spacing-1);
  line-height: 1;
}

.closeDetailButton:hover {
  color: var(--text-base);
}

.modalContent {
  padding: var(--spacing-4);
}

.formGroup {
  margin-bottom: var(--spacing-4);
}

.formLabel {
  display: block;
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  color: var(--text-subdued);
  margin-bottom: var(--spacing-2);
}

.formInput {
  width: 100%;
  padding: var(--spacing-3);
  background-color: var(--bg-base);
  border: 1px solid var(--border-subdued);
  border-radius: var(--radius-md);
  color: var(--text-base);
  font-size: var(--text-sm);
}

.formInput:focus {
  outline: none;
  border-color: var(--spotify-green);
}

.formInput::placeholder {
  color: var(--text-subdued);
}

.modalError {
  padding: var(--spacing-3);
  background-color: rgba(220, 38, 38, 0.1);
  border: 1px solid #dc2626;
  border-radius: var(--radius-md);
  color: #dc2626;
  font-size: var(--text-sm);
  margin-bottom: var(--spacing-4);
}

.modalSuccess {
  padding: var(--spacing-3);
  background-color: rgba(34, 197, 94, 0.1);
  border: 1px solid #22c55e;
  border-radius: var(--radius-md);
  color: #22c55e;
  font-size: var(--text-sm);
  margin-bottom: var(--spacing-4);
}

.modalActions {
  display: flex;
  justify-content: flex-end;
  gap: var(--spacing-3);
}

.cancelButton {
  padding: var(--spacing-2) var(--spacing-4);
  background: none;
  border: 1px solid var(--border-subdued);
  border-radius: var(--radius-md);
  color: var(--text-subdued);
  font-size: var(--text-sm);
  cursor: pointer;
}

.cancelButton:hover {
  border-color: var(--text-base);
  color: var(--text-base);
}

.confirmButton {
  padding: var(--spacing-2) var(--spacing-4);
  background-color: var(--spotify-green);
  border: none;
  border-radius: var(--radius-md);
  color: white;
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  cursor: pointer;
}

.confirmButton:hover:not(:disabled) {
  background-color: #1ed760;
}

.confirmButton:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

@media (max-width: 768px) {
  .actionButtons {
    flex-wrap: wrap;
  }

  .refreshButton {
    margin-left: 0;
    width: 100%;
  }

  .statsSummary {
    flex-direction: column;
    gap: var(--spacing-2);
  }
}

/* Statistics Tab */
.periodSelector {
  display: flex;
  gap: var(--spacing-2);
  margin-bottom: var(--spacing-4);
}

.periodButton {
  padding: var(--spacing-2) var(--spacing-4);
  background: none;
  border: 1px solid var(--border-subdued);
  border-radius: var(--radius-md);
  color: var(--text-subdued);
  font-size: var(--text-sm);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.periodButton:hover {
  border-color: var(--text-base);
  color: var(--text-base);
}

.periodButton.active {
  background-color: var(--spotify-green);
  border-color: var(--spotify-green);
  color: white;
}

.statsTotals {
  display: flex;
  flex-wrap: wrap;
  gap: var(--spacing-3);
  margin-bottom: var(--spacing-6);
}

.totalCard {
  flex: 1;
  min-width: 120px;
  padding: var(--spacing-4);
  background-color: var(--bg-elevated-base);
  border-radius: var(--radius-lg);
  text-align: center;
}

.totalValue {
  display: block;
  font-size: var(--text-2xl);
  font-weight: var(--font-bold);
  color: var(--text-base);
  margin-bottom: var(--spacing-1);
}

.totalLabel {
  font-size: var(--text-sm);
  color: var(--text-subdued);
}

.totalFailures .totalValue {
  color: #dc2626;
}

.chartSection {
  margin-bottom: var(--spacing-6);
}

.chartTitle {
  font-size: var(--text-base);
  font-weight: var(--font-semibold);
  color: var(--text-base);
  margin: 0 0 var(--spacing-3) 0;
}

.chartContainer {
  background-color: var(--bg-elevated-base);
  border-radius: var(--radius-lg);
  padding: var(--spacing-4);
  height: 300px;
}

.noData {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: var(--text-subdued);
}

.tableSection {
  margin-bottom: var(--spacing-6);
}

.tableWrapper {
  background-color: var(--bg-elevated-base);
  border-radius: var(--radius-lg);
  overflow-x: auto;
}

.dataTable {
  width: 100%;
  border-collapse: collapse;
  font-size: var(--text-sm);
}

.dataTable th,
.dataTable td {
  padding: var(--spacing-3) var(--spacing-4);
  text-align: left;
  border-bottom: 1px solid var(--border-subdued);
}

.dataTable th {
  font-weight: var(--font-semibold);
  color: var(--text-subdued);
  background-color: rgba(0, 0, 0, 0.2);
}

.dataTable td {
  color: var(--text-base);
}

.dataTable tr:last-child td {
  border-bottom: none;
}

.dataTable tr:hover td {
  background-color: var(--bg-highlight);
}

.text-danger {
  color: #dc2626;
  font-weight: var(--font-semibold);
}
</style>
