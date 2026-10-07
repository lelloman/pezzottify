<template>
  <div class="bugReports">
    <header class="pageHeader">
      <div>
        <h2 class="sectionTitle">Bug reports</h2>
        <p>
          Review issues reported by listeners, including logs and attachments.
        </p>
      </div>
      <button class="refreshButton" :disabled="isLoading" @click="loadReports">
        {{ isLoading ? "Loading…" : "Refresh" }}
      </button>
    </header>

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
        <div class="reportHeader">
          <button
            class="reportToggle"
            :aria-expanded="expandedReportId === report.id"
            :aria-controls="`report-details-${report.id}`"
            @click="toggleReport(report.id)"
          >
            <span class="reportInfo">
              <span class="reportTitle">
                {{ report.title || "(No title)" }}
              </span>
              <span class="reportMeta">
                <span class="reportUser">{{ report.user_handle }}</span>
                <span class="separator">•</span>
                <span class="clientBadge" :class="report.client_type">
                  {{ report.client_type }}
                </span>
                <span class="separator">•</span>
                <span class="reportDate">{{
                  formatDate(report.created_at)
                }}</span>
                <span class="separator">•</span>
                <span class="reportSize">{{
                  formatSize(report.size_bytes)
                }}</span>
              </span>
            </span>
            <svg
              class="expandIcon"
              :class="{ open: expandedReportId === report.id }"
              viewBox="0 0 24 24"
              aria-hidden="true"
            >
              <path d="m9 5 7 7-7 7" />
            </svg>
          </button>
          <div class="reportActions">
            <button
              class="deleteButton"
              :aria-label="`Delete report: ${report.title || report.id}`"
              :disabled="deletingId === report.id"
              @click.stop="confirmDelete(report)"
            >
              <svg viewBox="0 0 24 24" aria-hidden="true">
                <path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13M10 10v7m4-7v7" />
              </svg>
            </button>
          </div>
        </div>

        <div
          v-if="expandedReportId === report.id"
          class="reportDetails"
          :id="`report-details-${report.id}`"
        >
          <div v-if="loadingDetails" class="detailsLoading">
            Loading details...
          </div>
          <div v-else-if="detailsError" class="detailsError">
            {{ detailsError }}
          </div>
          <div v-else-if="expandedReport" class="detailsContent">
            <div class="detailSection wide">
              <h4 class="detailLabel">Description</h4>
              <div class="detailValue description">
                {{ expandedReport.description }}
              </div>
            </div>

            <div v-if="expandedReport.client_version" class="detailSection">
              <h4 class="detailLabel">Client Version</h4>
              <div class="detailValue">{{ expandedReport.client_version }}</div>
            </div>

            <div v-if="expandedReport.device_info" class="detailSection">
              <h4 class="detailLabel">Device Info</h4>
              <div class="detailValue">{{ expandedReport.device_info }}</div>
            </div>

            <div v-if="expandedReport.logs" class="detailSection wide">
              <h4 class="detailLabel">Logs</h4>
              <div
                class="detailValue logs"
                tabindex="0"
                role="region"
                aria-label="Report logs"
              >
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

            <div v-if="parsedAttachments.length > 0" class="detailSection wide">
              <h4 class="detailLabel">
                Attachments ({{ parsedAttachments.length }})
              </h4>
              <div class="attachmentsGrid">
                <button
                  v-for="(attachment, index) in parsedAttachments"
                  :key="index"
                  class="attachmentItem"
                  :aria-label="`Open attachment ${index + 1}`"
                  @click="openAttachment(attachment)"
                >
                  <img :src="attachment" alt="" class="attachmentThumb" />
                </button>
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
      <button class="pageButton" :disabled="offset === 0" @click="prevPage">
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
        <strong>{{ reportToDelete?.user_handle }}</strong
        >? This action cannot be undone.
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
  if (expandedReportId.value !== reportId) return;

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
    reports.value = reports.value.filter(
      (r) => r.id !== reportToDelete.value.id,
    );
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
  width: 100%;
  min-width: 0;
  color: var(--text-base);
}
.pageHeader {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 20px;
  margin-bottom: 28px;
}
.sectionTitle {
  margin: 0;
  font-size: 32px;
  font-weight: 700;
  letter-spacing: -0.04em;
}
.pageHeader p {
  margin: 8px 0 0;
  color: var(--text-subdued);
  font-size: 14px;
  line-height: 1.5;
}
button {
  font: inherit;
  cursor: pointer;
}
button:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}
button:focus-visible,
.logs:focus-visible {
  outline: 2px solid #fff;
  outline-offset: -2px;
}
.refreshButton,
.pageButton,
.retryButton {
  min-height: 40px;
  border: 1px solid #727272;
  border-radius: 999px;
  padding: 9px 20px;
  background: transparent;
  color: #fff;
  font-size: 13px;
  font-weight: 700;
}
.refreshButton:hover:not(:disabled),
.pageButton:hover:not(:disabled),
.retryButton:hover {
  border-color: #fff;
  background: #242424;
}
.loadingMessage,
.emptyMessage {
  padding: 40px 20px;
  text-align: center;
  background: #181818;
  border-radius: 8px;
  color: var(--text-subdued);
  font-size: 14px;
}
.errorMessage {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 16px;
  padding: 16px;
  margin-bottom: 20px;
  border-radius: 8px;
  background: #281a1d;
  color: #f3727f;
  font-size: 14px;
}
.reportsList {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.reportCard {
  background: #181818;
  border-radius: 8px;
  overflow: hidden;
}
.reportHeader {
  display: flex;
  align-items: center;
}
.reportToggle {
  display: flex;
  align-items: center;
  gap: 24px;
  flex: 1;
  min-width: 0;
  padding: 20px;
  border: 0;
  text-align: left;
  background: transparent;
  color: inherit;
}
.reportToggle:hover {
  background: #242424;
}
.reportInfo {
  display: block;
  flex: 1;
  min-width: 0;
}
.reportTitle {
  display: block;
  margin-bottom: 10px;
  font-size: 16px;
  font-weight: 600;
  overflow-wrap: anywhere;
}
.reportMeta {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
  color: var(--text-subdued);
  font-size: 12px;
}
.separator {
  color: #727272;
}
.clientBadge {
  padding: 3px 8px;
  border-radius: 4px;
  background: #2a2a2a;
  color: #b3b3b3;
  text-transform: capitalize;
}
.reportActions {
  padding-right: 12px;
}
.deleteButton {
  display: grid;
  place-items: center;
  width: 40px;
  height: 40px;
  border: 0;
  border-radius: 50%;
  background: transparent;
  color: #b3b3b3;
}
.deleteButton:hover:not(:disabled) {
  background: #302024;
  color: #f3727f;
}
.deleteButton svg,
.expandIcon {
  width: 20px;
  height: 20px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.6;
  flex-shrink: 0;
}
.expandIcon {
  width: 16px;
  height: 16px;
  color: #b3b3b3;
  transition: transform 0.15s;
}
.expandIcon.open {
  transform: rotate(90deg);
}
.reportDetails {
  padding: 24px;
  border-top: 1px solid #333;
}
.detailsContent {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 24px;
}
.detailSection {
  min-width: 0;
}
.detailSection.wide {
  grid-column: 1 / -1;
}
.detailsLoading,
.detailsError {
  font-size: 14px;
  color: var(--text-subdued);
}
.detailsError {
  color: #f3727f;
}
.detailLabel {
  margin: 0 0 8px;
  color: var(--text-subdued);
  font-size: 13px;
  font-weight: 600;
}
.detailValue {
  font-size: 14px;
  line-height: 1.6;
  overflow-wrap: anywhere;
}
.description {
  white-space: pre-wrap;
}
.logs {
  padding: 16px;
  background: #101010;
  border-radius: 4px;
  max-height: 320px;
  overflow: auto;
}
.logs pre {
  margin: 0;
  font-family: monospace;
  font-size: 12px;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
.mono {
  font-family: monospace;
  font-size: 12px;
  color: var(--text-subdued);
}
.showMoreButton {
  margin-top: 12px;
  padding: 8px 0;
  color: #fff;
  background: none;
  border: 0;
  font-size: 13px;
  font-weight: 700;
}
.showMoreButton:hover {
  text-decoration: underline;
}
.attachmentsGrid {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
}
.attachmentItem {
  padding: 0;
  width: 120px;
  height: 90px;
  border-radius: 4px;
  border: 1px solid #535353;
  overflow: hidden;
  background: #101010;
}
.attachmentItem:hover {
  border-color: #fff;
}
.attachmentThumb {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.pagination {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 16px;
  margin-top: 24px;
  padding-top: 20px;
  border-top: 1px solid #282828;
  flex-wrap: wrap;
}
.pageInfo {
  font-size: 13px;
  color: var(--text-subdued);
}
@media (max-width: 600px) {
  .pageHeader {
    align-items: flex-start;
    flex-direction: column;
  }
  .reportToggle {
    padding: 16px;
    gap: 12px;
  }
  .reportActions {
    padding-right: 8px;
  }
  .reportDetails {
    padding: 16px;
  }
  .detailsContent {
    grid-template-columns: minmax(0, 1fr);
  }
  .pagination {
    justify-content: center;
    gap: 10px;
  }
  .pageButton {
    padding: 9px 14px;
  }
}
</style>
