<template>
  <div class="downloadManager">
    <h2 class="sectionTitle">Download Manager</h2>

    <!-- Stats Overview -->
    <div class="statsGrid">
      <div class="statCard">
        <span class="statValue">{{ stats?.queue?.pending ?? "—" }}</span>
        <span class="statLabel">Pending</span>
      </div>
      <div class="statCard">
        <span class="statValue">{{ stats?.queue?.in_progress ?? "—" }}</span>
        <span class="statLabel">In Progress</span>
      </div>
      <div class="statCard">
        <span class="statValue">{{ stats?.queue?.retry_waiting ?? "—" }}</span>
        <span class="statLabel">Retry Waiting</span>
      </div>
      <div class="statCard success">
        <span class="statValue">{{ stats?.queue?.completed_today ?? "—" }}</span>
        <span class="statLabel">Completed Today</span>
      </div>
      <div class="statCard danger">
        <span class="statValue">{{ stats?.queue?.failed_today ?? "—" }}</span>
        <span class="statLabel">Failed Today</span>
      </div>
    </div>

    <!-- Capacity Info -->
    <div class="capacityCard">
      <h3 class="cardTitle">Rate Limits</h3>
      <div class="capacityGrid">
        <div class="capacityItem">
          <span class="capacityValue"
            >{{ stats?.capacity?.albums_this_hour ?? "—" }} /
            {{ stats?.capacity?.max_per_hour ?? "—" }}</span
          >
          <span class="capacityLabel">Albums this hour</span>
        </div>
        <div class="capacityItem">
          <span class="capacityValue"
            >{{ stats?.capacity?.albums_today ?? "—" }} /
            {{ stats?.capacity?.max_per_day ?? "—" }}</span
          >
          <span class="capacityLabel">Albums today</span>
        </div>
      </div>
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
      </button>
      <button class="refreshButton" @click="loadData" :disabled="isLoading">
        {{ isLoading ? "Loading..." : "Refresh" }}
      </button>
    </div>

    <div v-if="loadError" class="errorMessage">
      {{ loadError }}
    </div>

    <!-- Failed Downloads Tab -->
    <div v-if="activeTab === 'failed'" class="tabContent">
      <div v-if="failedItems.length === 0" class="emptyState">
        No failed downloads.
      </div>
      <div v-else class="tableWrapper">
        <table class="dataTable">
          <thead>
            <tr>
              <th>External ID</th>
              <th>Type</th>
              <th>Retries</th>
              <th>Error</th>
              <th>Failed At</th>
              <th>Action</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="item in failedItems" :key="item.id">
              <td class="idCell">{{ item.external_id }}</td>
              <td>{{ item.item_type }}</td>
              <td>{{ item.retry_count }}</td>
              <td class="errorCell">{{ item.last_error || "—" }}</td>
              <td>{{ formatDate(item.updated_at) }}</td>
              <td>
                <button
                  class="retryButton"
                  @click="handleRetry(item.id)"
                  :disabled="retryingItems[item.id]"
                >
                  {{ retryingItems[item.id] ? "..." : "Retry" }}
                </button>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <!-- Activity Tab -->
    <div v-if="activeTab === 'activity'" class="tabContent">
      <div v-if="activity.length === 0" class="emptyState">
        No recent activity.
      </div>
      <div v-else class="tableWrapper">
        <table class="dataTable">
          <thead>
            <tr>
              <th>External ID</th>
              <th>Type</th>
              <th>Status</th>
              <th>User</th>
              <th>Updated</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="item in activity" :key="item.id">
              <td class="idCell">{{ item.external_id }}</td>
              <td>{{ item.item_type }}</td>
              <td>
                <span class="statusBadge" :class="statusClass(item.status)">
                  {{ item.status }}
                </span>
              </td>
              <td>{{ item.user_handle || "—" }}</td>
              <td>{{ formatDate(item.updated_at) }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <!-- All Requests Tab -->
    <div v-if="activeTab === 'requests'" class="tabContent">
      <div v-if="requests.length === 0" class="emptyState">
        No download requests.
      </div>
      <div v-else class="tableWrapper">
        <table class="dataTable">
          <thead>
            <tr>
              <th>External ID</th>
              <th>Type</th>
              <th>Status</th>
              <th>User</th>
              <th>Requested</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="item in requests" :key="item.id">
              <td class="idCell">{{ item.external_id }}</td>
              <td>{{ item.item_type }}</td>
              <td>
                <span class="statusBadge" :class="statusClass(item.status)">
                  {{ item.status }}
                </span>
              </td>
              <td>{{ item.user_handle || "—" }}</td>
              <td>{{ formatDate(item.created_at) }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <!-- Audit Log Tab -->
    <div v-if="activeTab === 'audit'" class="tabContent">
      <div v-if="auditLog.length === 0" class="emptyState">
        No audit log entries.
      </div>
      <div v-else class="tableWrapper">
        <table class="dataTable">
          <thead>
            <tr>
              <th>Time</th>
              <th>Event</th>
              <th>User</th>
              <th>External ID</th>
              <th>Details</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="entry in auditLog" :key="entry.id">
              <td>{{ formatDate(entry.created_at) }}</td>
              <td>
                <span class="eventBadge" :class="eventClass(entry.event_type)">
                  {{ formatEventType(entry.event_type) }}
                </span>
              </td>
              <td>{{ entry.user_handle || "system" }}</td>
              <td class="idCell">{{ entry.external_id || "—" }}</td>
              <td class="detailsCell">{{ entry.details || "—" }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, reactive, onMounted, onUnmounted } from "vue";
import { useRemoteStore } from "@/store/remote";

const remoteStore = useRemoteStore();

const tabs = [
  { id: "failed", label: "Failed" },
  { id: "activity", label: "Activity" },
  { id: "requests", label: "All Requests" },
  { id: "audit", label: "Audit Log" },
];

const activeTab = ref("failed");
const isLoading = ref(false);
const loadError = ref(null);

const stats = ref(null);
const failedItems = ref([]);
const activity = ref([]);
const requests = ref([]);
const auditLog = ref([]);
const retryingItems = reactive({});

const loadData = async () => {
  isLoading.value = true;
  loadError.value = null;

  try {
    const [statsResult, failedResult, activityResult, requestsResult, auditResult] =
      await Promise.all([
        remoteStore.fetchDownloadStats(),
        remoteStore.fetchFailedDownloads(50, 0),
        remoteStore.fetchDownloadActivity(50),
        remoteStore.fetchDownloadRequests(100, 0),
        remoteStore.fetchDownloadAuditLog(100, 0),
      ]);

    stats.value = statsResult;
    failedItems.value = failedResult?.items || [];
    activity.value = activityResult?.items || [];
    requests.value = requestsResult?.items || [];
    auditLog.value = auditResult?.entries || [];

    if (!statsResult) {
      loadError.value = "Download manager may not be enabled on this server.";
    }
  } catch {
    loadError.value = "Failed to load download manager data.";
  }

  isLoading.value = false;
};

const handleRetry = async (itemId) => {
  retryingItems[itemId] = true;

  const result = await remoteStore.retryDownload(itemId);

  if (result.error) {
    alert(result.error);
  } else {
    // Reload data to see updated status
    await loadData();
  }

  retryingItems[itemId] = false;
};

const formatDate = (timestamp) => {
  if (!timestamp) return "—";
  const date = new Date(timestamp);
  return date.toLocaleString();
};

const statusClass = (status) => {
  switch (status?.toLowerCase()) {
    case "completed":
      return "status-completed";
    case "in_progress":
      return "status-progress";
    case "pending":
      return "status-pending";
    case "failed":
      return "status-failed";
    case "retry_waiting":
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
  // Convert snake_case to Title Case
  return eventType
    .split("_")
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(" ");
};

// Auto-refresh every 30 seconds
const REFRESH_INTERVAL = 30000;
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
  margin: 0 0 var(--spacing-6) 0;
}

/* Stats Grid */
.statsGrid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
  gap: var(--spacing-3);
  margin-bottom: var(--spacing-4);
}

.statCard {
  background-color: var(--bg-elevated-base);
  border-radius: var(--radius-lg);
  padding: var(--spacing-4);
  text-align: center;
}

.statCard.success .statValue {
  color: var(--spotify-green);
}

.statCard.danger .statValue {
  color: #dc2626;
}

.statValue {
  display: block;
  font-size: var(--text-2xl);
  font-weight: var(--font-bold);
  color: var(--text-base);
}

.statLabel {
  display: block;
  font-size: var(--text-sm);
  color: var(--text-subdued);
  margin-top: var(--spacing-1);
}

/* Capacity Card */
.capacityCard {
  background-color: var(--bg-elevated-base);
  border-radius: var(--radius-lg);
  padding: var(--spacing-4);
  margin-bottom: var(--spacing-4);
}

.cardTitle {
  font-size: var(--text-base);
  font-weight: var(--font-semibold);
  color: var(--text-base);
  margin: 0 0 var(--spacing-3) 0;
}

.capacityGrid {
  display: flex;
  gap: var(--spacing-6);
  flex-wrap: wrap;
}

.capacityItem {
  display: flex;
  flex-direction: column;
}

.capacityValue {
  font-size: var(--text-lg);
  font-weight: var(--font-medium);
  color: var(--text-base);
}

.capacityLabel {
  font-size: var(--text-sm);
  color: var(--text-subdued);
}

/* Tab Navigation */
.tabNav {
  display: flex;
  flex-wrap: wrap;
  gap: var(--spacing-2);
  margin-bottom: var(--spacing-4);
  border-bottom: 1px solid var(--border-subdued);
  padding-bottom: var(--spacing-2);
}

.tabButton {
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

.refreshButton {
  margin-left: auto;
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

.refreshButton:hover:not(:disabled) {
  background-color: #1ed760;
}

.refreshButton:disabled {
  opacity: 0.6;
  cursor: not-allowed;
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

/* Table Styles */
.tableWrapper {
  overflow-x: auto;
}

.dataTable {
  width: 100%;
  border-collapse: collapse;
  background-color: var(--bg-elevated-base);
  border-radius: var(--radius-lg);
  overflow: hidden;
}

.dataTable th,
.dataTable td {
  padding: var(--spacing-3) var(--spacing-4);
  text-align: left;
  border-bottom: 1px solid var(--border-subdued);
}

.dataTable th {
  background-color: var(--bg-highlight);
  color: var(--text-subdued);
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.dataTable td {
  color: var(--text-base);
  font-size: var(--text-sm);
}

.dataTable tr:last-child td {
  border-bottom: none;
}

.dataTable tr:hover td {
  background-color: var(--bg-highlight);
}

.idCell {
  font-family: monospace;
  font-size: var(--text-xs);
  max-width: 200px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.errorCell {
  max-width: 250px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: #dc2626;
}

.detailsCell {
  max-width: 300px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--text-xs);
  color: var(--text-subdued);
}

/* Status Badge */
.statusBadge {
  display: inline-block;
  padding: var(--spacing-1) var(--spacing-2);
  border-radius: var(--radius-full);
  font-size: var(--text-xs);
  font-weight: var(--font-medium);
  text-transform: lowercase;
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

/* Event Badge */
.eventBadge {
  display: inline-block;
  padding: var(--spacing-1) var(--spacing-2);
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

/* Retry Button */
.retryButton {
  padding: var(--spacing-1) var(--spacing-3);
  background-color: var(--spotify-green);
  color: white;
  border: none;
  border-radius: var(--radius-md);
  font-size: var(--text-xs);
  font-weight: var(--font-medium);
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

@media (max-width: 768px) {
  .statsGrid {
    grid-template-columns: repeat(2, 1fr);
  }

  .capacityGrid {
    flex-direction: column;
    gap: var(--spacing-2);
  }

  .tabNav {
    flex-direction: column;
  }

  .refreshButton {
    margin-left: 0;
    width: 100%;
  }
}
</style>
