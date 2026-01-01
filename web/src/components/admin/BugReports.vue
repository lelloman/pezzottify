<template>
  <div class="bugReports">
    <h2 class="sectionTitle">Bug Reports</h2>

    <div class="actionButtons">
      <button class="refreshButton" :disabled="isLoading" @click="loadReports">
        {{ isLoading ? "Loading..." : "Refresh" }}
      </button>
    </div>

    <div v-if="deleteError" class="errorMessage">
      {{ deleteError }}
      <button class="retryButton" @click="deleteError = null">Dismiss</button>
    </div>

    <div v-if="isLoading && reports.length === 0" class="loadingMessage">
      Loading bug reports...
    </div>
    <div v-else-if="loadError" class="errorMessage">
      {{ loadError }}
      <button class="retryButton" @click="loadReports">Retry</button>
    </div>
    <div v-else-if="reports.length === 0" class="emptyMessage">
      No bug reports submitted yet.
    </div>

    <div v-else class="reportsList">
      <div
        v-for="report in reports"
        :key="report.id"
        class="reportCard"
        :class="{ expanded: expandedReportId === report.id }"
      >
        <div class="reportHeader" @click="toggleReport(report.id)">
          <div class="reportInfo">
            <div class="reportTitle">
              {{ report.title || "(No title)" }}
            </div>
            <div class="reportMeta">
              <span class="reportUser">{{ report.user_handle }}</span>
              <span class="separator">•</span>
              <span class="clientBadge" :class="report.client_type">
                {{ report.client_type }}
              </span>
              <span class="separator">•</span>
              <span class="reportDate">{{ formatDate(report.created_at) }}</span>
              <span class="separator">•</span>
              <span class="reportSize">{{ formatSize(report.size_bytes) }}</span>
            </div>
          </div>
          <div class="reportActions">
            <button
              class="deleteButton"
              :disabled="deletingId === report.id"
              @click.stop="confirmDelete(report)"
            >
              {{ deletingId === report.id ? "Deleting..." : "Delete" }}
            </button>
            <span class="expandIcon">{{ expandedReportId === report.id ? "▼" : "▶" }}</span>
          </div>
        </div>

        <div v-if="expandedReportId === report.id" class="reportDetails">
          <div v-if="loadingDetails" class="detailsLoading">Loading details...</div>
          <div v-else-if="detailsError" class="detailsError">{{ detailsError }}</div>
          <div v-else-if="expandedReport" class="detailsContent">
            <div class="detailSection">
              <h4 class="detailLabel">Description</h4>
              <div class="detailValue description">{{ expandedReport.description }}</div>
            </div>

            <div v-if="expandedReport.client_version" class="detailSection">
              <h4 class="detailLabel">Client Version</h4>
              <div class="detailValue">{{ expandedReport.client_version }}</div>
            </div>

            <div v-if="expandedReport.device_info" class="detailSection">
              <h4 class="detailLabel">Device Info</h4>
              <div class="detailValue">{{ expandedReport.device_info }}</div>
            </div>

            <div v-if="expandedReport.logs" class="detailSection">
              <h4 class="detailLabel">Logs</h4>
              <div class="detailValue logs">
                <pre>{{ truncateLogs(expandedReport.logs) }}</pre>
                <button
                  v-if="expandedReport.logs.length > 2000"
                  class="showMoreButton"
                  @click="showFullLogs = !showFullLogs"
                >
                  {{ showFullLogs ? "Show less" : "Show more" }}
                </button>
              </div>
            </div>

            <div v-if="parsedAttachments.length > 0" class="detailSection">
              <h4 class="detailLabel">Attachments ({{ parsedAttachments.length }})</h4>
              <div class="attachmentsGrid">
                <div
                  v-for="(attachment, index) in parsedAttachments"
                  :key="index"
                  class="attachmentItem"
                  @click="openAttachment(attachment)"
                >
                  <img :src="attachment" alt="Attachment" class="attachmentThumb" />
                </div>
              </div>
            </div>

            <div class="detailSection">
              <h4 class="detailLabel">Report ID</h4>
              <div class="detailValue mono">{{ expandedReport.id }}</div>
            </div>
          </div>
        </div>
      </div>
    </div>

    <div v-if="reports.length > 0" class="pagination">
      <button
        class="pageButton"
        :disabled="offset === 0"
        @click="prevPage"
      >
        Previous
      </button>
      <span class="pageInfo">
        Showing {{ offset + 1 }}-{{ offset + reports.length }}
      </span>
      <button
        class="pageButton"
        :disabled="reports.length < limit"
        @click="nextPage"
      >
        Next
      </button>
    </div>

    <ConfirmationDialog
      :isOpen="showConfirmDialog"
      :closeCallback="() => (showConfirmDialog = false)"
      :positiveButtonCallback="handleDelete"
      title="Delete Bug Report"
      positiveButtonText="Delete"
      negativeButtonText="Cancel"
    >
      <template #message>
        Are you sure you want to delete this bug report from
        <strong>{{ reportToDelete?.user_handle }}</strong>?
        This action cannot be undone.
      </template>
    </ConfirmationDialog>
  </div>
</template>

<script setup>
import { ref, computed, onMounted } from "vue";
import { useRemoteStore } from "@/store/remote";
import ConfirmationDialog from "@/components/common/ConfirmationDialog.vue";

const remoteStore = useRemoteStore();

const reports = ref([]);
const isLoading = ref(true);
const loadError = ref(null);

const limit = ref(20);
const offset = ref(0);

const expandedReportId = ref(null);
const expandedReport = ref(null);
const loadingDetails = ref(false);
const detailsError = ref(null);
const showFullLogs = ref(false);

const showConfirmDialog = ref(false);
const reportToDelete = ref(null);
const deletingId = ref(null);
const deleteError = ref(null);

const parsedAttachments = computed(() => {
  if (!expandedReport.value?.attachments) return [];
  try {
    const attachments = JSON.parse(expandedReport.value.attachments);
    return attachments.map((base64) => {
      // Assume JPEG if no prefix, otherwise use as-is
      if (base64.startsWith("data:")) return base64;
      return `data:image/jpeg;base64,${base64}`;
    });
  } catch {
    return [];
  }
});

const loadReports = async () => {
  isLoading.value = true;
  loadError.value = null;

  const result = await remoteStore.fetchBugReports(limit.value, offset.value);
  if (result === null) {
    loadError.value = "Failed to load bug reports. Please try again.";
  } else {
    reports.value = result;
  }

  isLoading.value = false;
};

const toggleReport = async (reportId) => {
  if (expandedReportId.value === reportId) {
    expandedReportId.value = null;
    expandedReport.value = null;
    return;
  }

  expandedReportId.value = reportId;
  expandedReport.value = null;
  loadingDetails.value = true;
  detailsError.value = null;
  showFullLogs.value = false;

  const result = await remoteStore.getBugReport(reportId);
  if (result === null) {
    detailsError.value = "Failed to load report details.";
  } else {
    expandedReport.value = result;
  }

  loadingDetails.value = false;
};

const confirmDelete = (report) => {
  reportToDelete.value = report;
  showConfirmDialog.value = true;
};

const handleDelete = async () => {
  if (!reportToDelete.value) return;

  showConfirmDialog.value = false;
  deletingId.value = reportToDelete.value.id;
  deleteError.value = null;

  const result = await remoteStore.deleteBugReport(reportToDelete.value.id);

  if (result.success) {
    // Remove from list
    reports.value = reports.value.filter((r) => r.id !== reportToDelete.value.id);
    if (expandedReportId.value === reportToDelete.value.id) {
      expandedReportId.value = null;
      expandedReport.value = null;
    }
  } else {
    deleteError.value = result.error;
  }

  deletingId.value = null;
  reportToDelete.value = null;
};

const formatDate = (dateStr) => {
  const date = new Date(dateStr);
  return date.toLocaleString();
};

const formatSize = (bytes) => {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
};

const truncateLogs = (logs) => {
  if (showFullLogs.value || logs.length <= 2000) return logs;
  return logs.substring(0, 2000) + "...";
};

const openAttachment = (dataUrl) => {
  window.open(dataUrl, "_blank");
};

const prevPage = () => {
  offset.value = Math.max(0, offset.value - limit.value);
  loadReports();
};

const nextPage = () => {
  offset.value += limit.value;
  loadReports();
};

onMounted(() => {
  loadReports();
});
</script>

<style scoped>
.bugReports {
  max-width: 900px;
}

.sectionTitle {
  font-size: var(--text-2xl);
  font-weight: var(--font-bold);
  color: var(--text-base);
  margin: 0 0 var(--spacing-6) 0;
}

.actionButtons {
  display: flex;
  gap: var(--spacing-2);
  margin-bottom: var(--spacing-4);
}

.refreshButton {
  padding: var(--spacing-2) var(--spacing-4);
  background-color: var(--highlight);
  color: var(--text-base);
  border: none;
  border-radius: var(--radius-md);
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  cursor: pointer;
  transition: background-color var(--transition-fast);
}

.refreshButton:hover:not(:disabled) {
  filter: brightness(1.1);
}

.refreshButton:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.loadingMessage,
.emptyMessage {
  padding: var(--spacing-4);
  color: var(--text-subdued);
  font-size: var(--text-sm);
}

.errorMessage {
  padding: var(--spacing-3) var(--spacing-4);
  background-color: rgba(220, 38, 38, 0.1);
  border: 1px solid #dc2626;
  border-radius: var(--radius-md);
  color: #dc2626;
  font-size: var(--text-sm);
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
}

.retryButton {
  padding: var(--spacing-1) var(--spacing-3);
  background-color: #dc2626;
  color: white;
  border: none;
  border-radius: var(--radius-md);
  font-size: var(--text-sm);
  cursor: pointer;
}

.reportsList {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-3);
}

.reportCard {
  background-color: var(--bg-elevated-base);
  border-radius: var(--radius-lg);
  overflow: hidden;
}

.reportHeader {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: var(--spacing-4);
  cursor: pointer;
  transition: background-color var(--transition-fast);
}

.reportHeader:hover {
  background-color: var(--bg-highlight);
}

.reportInfo {
  flex: 1;
  min-width: 0;
}

.reportTitle {
  font-size: var(--text-base);
  font-weight: var(--font-semibold);
  color: var(--text-base);
  margin-bottom: var(--spacing-1);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.reportMeta {
  font-size: var(--text-xs);
  color: var(--text-subdued);
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  flex-wrap: wrap;
}

.separator {
  color: var(--border-subdued);
}

.clientBadge {
  padding: 2px 6px;
  border-radius: var(--radius-sm);
  font-size: var(--text-xs);
  font-weight: var(--font-medium);
  text-transform: uppercase;
}

.clientBadge.android {
  background-color: rgba(61, 220, 132, 0.2);
  color: #3ddc84;
}

.clientBadge.web {
  background-color: rgba(59, 130, 246, 0.2);
  color: #3b82f6;
}

.reportActions {
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
  flex-shrink: 0;
}

.deleteButton {
  padding: var(--spacing-1) var(--spacing-3);
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

.expandIcon {
  color: var(--text-subdued);
  font-size: var(--text-xs);
}

.reportDetails {
  padding: var(--spacing-4);
  border-top: 1px solid var(--border-subdued);
  background-color: var(--bg-base);
}

.detailsLoading,
.detailsError {
  font-size: var(--text-sm);
  color: var(--text-subdued);
  padding: var(--spacing-2);
}

.detailsError {
  color: #dc2626;
}

.detailSection {
  margin-bottom: var(--spacing-4);
}

.detailSection:last-child {
  margin-bottom: 0;
}

.detailLabel {
  font-size: var(--text-xs);
  font-weight: var(--font-semibold);
  color: var(--text-subdued);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  margin: 0 0 var(--spacing-1) 0;
}

.detailValue {
  font-size: var(--text-sm);
  color: var(--text-base);
  line-height: 1.5;
}

.detailValue.description {
  white-space: pre-wrap;
  word-break: break-word;
}

.detailValue.logs {
  background-color: var(--bg-elevated-base);
  padding: var(--spacing-3);
  border-radius: var(--radius-md);
  max-height: 300px;
  overflow-y: auto;
}

.detailValue.logs pre {
  margin: 0;
  font-family: monospace;
  font-size: var(--text-xs);
  white-space: pre-wrap;
  word-break: break-all;
}

.detailValue.mono {
  font-family: monospace;
  font-size: var(--text-xs);
  color: var(--text-subdued);
}

.showMoreButton {
  margin-top: var(--spacing-2);
  padding: var(--spacing-1) var(--spacing-2);
  background-color: transparent;
  color: var(--highlight);
  border: none;
  font-size: var(--text-xs);
  cursor: pointer;
}

.attachmentsGrid {
  display: flex;
  gap: var(--spacing-2);
  flex-wrap: wrap;
}

.attachmentItem {
  width: 80px;
  height: 80px;
  border-radius: var(--radius-md);
  overflow: hidden;
  cursor: pointer;
  border: 1px solid var(--border-subdued);
  transition: border-color var(--transition-fast);
}

.attachmentItem:hover {
  border-color: var(--highlight);
}

.attachmentThumb {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.pagination {
  display: flex;
  justify-content: center;
  align-items: center;
  gap: var(--spacing-4);
  margin-top: var(--spacing-6);
}

.pageButton {
  padding: var(--spacing-2) var(--spacing-4);
  background-color: var(--bg-elevated-base);
  color: var(--text-base);
  border: 1px solid var(--border-subdued);
  border-radius: var(--radius-md);
  font-size: var(--text-sm);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.pageButton:hover:not(:disabled) {
  background-color: var(--bg-highlight);
  border-color: var(--border-default);
}

.pageButton:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.pageInfo {
  font-size: var(--text-sm);
  color: var(--text-subdued);
}
</style>
