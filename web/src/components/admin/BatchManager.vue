<template>
  <div class="batchManager">
    <header class="pageHeader">
      <h2 class="sectionTitle">Catalog batches</h2>
      <p>Group catalog changes and review what each batch contains.</p>
    </header>

    <!-- Action Buttons -->
    <div class="actionButtons">
      <button class="actionButton" @click="openCreateModal">New batch</button>
      <button class="refreshButton" @click="loadData" :disabled="isLoading">
        {{ isLoading ? "Loading..." : "Refresh" }}
      </button>
    </div>

    <!-- Stats Summary -->
    <div class="statsSummary">
      <span class="statItem">
        <strong>{{ openBatches.length }}</strong> open
      </span>
      <span class="statItem">
        <strong>{{ closedBatches.length }}</strong> closed
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

    <div v-if="loadError" class="errorMessage">
      {{ loadError }}
    </div>

    <!-- Open Batches Tab -->
    <div v-if="activeTab === 'open'" class="tabContent">
      <div v-if="openBatches.length === 0" class="emptyState">
        No open batches.
      </div>
      <div v-else class="batchList">
        <div
          v-for="batch in openBatches"
          :key="batch.id"
          class="batchItem status-open"
        >
          <div class="batchItemHeader">
            <div class="batchItemMain">
              <span class="batchItemName">{{ batch.name }}</span>
              <span class="statusBadge status-open">open</span>
            </div>
            <div class="batchItemActions">
              <button class="viewButton" @click="viewBatchChanges(batch)">
                View changes
              </button>
              <button
                class="closeButton"
                @click="confirmCloseBatch(batch)"
                :disabled="closingBatches[batch.id]"
              >
                {{ closingBatches[batch.id] ? "..." : "Close" }}
              </button>
              <button
                class="deleteButton"
                @click="confirmDeleteBatch(batch)"
                :disabled="deletingBatches[batch.id]"
              >
                {{ deletingBatches[batch.id] ? "..." : "Delete" }}
              </button>
            </div>
          </div>
          <div v-if="batch.description" class="batchDescription">
            {{ batch.description }}
          </div>
          <div class="batchItemDetails">
            <span class="detailItem">
              <span class="detailLabel">Created:</span>
              <span class="detailValue">{{
                formatDate(batch.created_at)
              }}</span>
            </span>
            <span class="detailItem">
              <span class="detailLabel">Last activity:</span>
              <span class="detailValue">{{
                formatDate(batch.last_activity_at)
              }}</span>
            </span>
          </div>
        </div>
      </div>
    </div>

    <!-- Closed Batches Tab -->
    <div v-if="activeTab === 'closed'" class="tabContent">
      <div v-if="closedBatches.length === 0" class="emptyState">
        No closed batches.
      </div>
      <div v-else class="batchList">
        <div
          v-for="batch in closedBatches"
          :key="batch.id"
          class="batchItem status-closed"
        >
          <div class="batchItemHeader">
            <div class="batchItemMain">
              <span class="batchItemName">{{ batch.name }}</span>
              <span class="statusBadge status-closed">closed</span>
            </div>
            <div class="batchItemActions">
              <button class="viewButton" @click="viewBatchChanges(batch)">
                View changes
              </button>
            </div>
          </div>
          <div v-if="batch.description" class="batchDescription">
            {{ batch.description }}
          </div>
          <div class="batchItemDetails">
            <span class="detailItem">
              <span class="detailLabel">Created:</span>
              <span class="detailValue">{{
                formatDate(batch.created_at)
              }}</span>
            </span>
            <span class="detailItem">
              <span class="detailLabel">Closed:</span>
              <span class="detailValue">{{ formatDate(batch.closed_at) }}</span>
            </span>
          </div>
        </div>
      </div>
    </div>

    <!-- Create Batch Modal -->
    <div
      v-if="showCreateModal"
      class="detailOverlay"
      @click.self="closeCreateModal"
    >
      <div
        class="detailPanel createModal"
        role="dialog"
        aria-modal="true"
        aria-label="Create batch"
      >
        <div class="detailHeader">
          <h3 class="detailTitle">Create New batch</h3>
          <button
            class="closeDetailButton"
            aria-label="Close create dialog"
            @click="closeCreateModal"
          >
            ×
          </button>
        </div>
        <div class="modalContent">
          <div class="formGroup">
            <label class="formLabel" for="batch-name">Batch name</label>
            <input
              id="batch-name"
              v-model="createForm.name"
              type="text"
              class="formInput"
              placeholder="e.g. Autumn arrivals"
            />
          </div>
          <div class="formGroup">
            <label class="formLabel" for="batch-description"
              >Description (optional)</label
            >
            <textarea
              id="batch-description"
              v-model="createForm.description"
              class="formInput formTextarea"
              placeholder="Brief description of the batch contents"
            ></textarea>
          </div>
          <div v-if="createError" class="modalError">
            {{ createError }}
          </div>
          <div class="modalActions">
            <button class="cancelButton" @click="closeCreateModal">
              Cancel
            </button>
            <button
              class="confirmButton"
              @click="submitCreateBatch"
              :disabled="isCreating || !createForm.name"
            >
              {{ isCreating ? "Creating..." : "Create" }}
            </button>
          </div>
        </div>
      </div>
    </div>

    <!-- Close batch Confirmation Modal -->
    <div
      v-if="showCloseModal"
      class="detailOverlay"
      @click.self="closeCloseModal"
    >
      <div
        class="detailPanel closeModal"
        role="dialog"
        aria-modal="true"
        aria-label="Close batch"
      >
        <div class="detailHeader">
          <h3 class="detailTitle">Close batch</h3>
          <button
            class="closeDetailButton"
            aria-label="Close close dialog"
            @click="closeCloseModal"
          >
            ×
          </button>
        </div>
        <div class="modalContent">
          <p class="closeWarning">
            Are you sure you want to close this batch? Once closed, no more
            changes can be added to it.
          </p>
          <div class="batchInfo">
            <span class="batchItemName">{{ batchToClose?.name }}</span>
          </div>
          <div v-if="closeError" class="modalError">
            {{ closeError }}
          </div>
          <div class="modalActions">
            <button class="cancelButton" @click="closeCloseModal">
              Cancel
            </button>
            <button
              class="confirmButton"
              @click="executeCloseBatch"
              :disabled="isClosing"
            >
              {{ isClosing ? "Closing..." : "Close batch" }}
            </button>
          </div>
        </div>
      </div>
    </div>

    <!-- Delete batch Confirmation Modal -->
    <div
      v-if="showDeleteModal"
      class="detailOverlay"
      @click.self="closeDeleteModal"
    >
      <div
        class="detailPanel deleteModal"
        role="dialog"
        aria-modal="true"
        aria-label="Delete batch"
      >
        <div class="detailHeader">
          <h3 class="detailTitle">Delete batch</h3>
          <button
            class="closeDetailButton"
            aria-label="Close delete dialog"
            @click="closeDeleteModal"
          >
            ×
          </button>
        </div>
        <div class="modalContent">
          <p class="deleteWarning">
            Are you sure you want to delete this batch? This action cannot be
            undone. Note: Only empty batches can be deleted.
          </p>
          <div class="batchInfo">
            <span class="batchItemName">{{ batchToDelete?.name }}</span>
          </div>
          <div v-if="deleteError" class="modalError">
            {{ deleteError }}
          </div>
          <div class="modalActions">
            <button class="cancelButton" @click="closeDeleteModal">
              Cancel
            </button>
            <button
              class="deleteConfirmButton"
              @click="executeDeleteBatch"
              :disabled="isDeleting"
            >
              {{ isDeleting ? "Deleting..." : "Delete" }}
            </button>
          </div>
        </div>
      </div>
    </div>

    <!-- View changes Modal -->
    <div
      v-if="showChangesModal"
      class="detailOverlay"
      @click.self="closeChangesModal"
    >
      <div
        class="detailPanel changesModal"
        role="dialog"
        aria-modal="true"
        aria-label="Batch changes"
      >
        <div class="detailHeader">
          <h3 class="detailTitle">Changes: {{ viewingBatch?.name }}</h3>
          <button
            class="closeDetailButton"
            aria-label="Close changes dialog"
            @click="closeChangesModal"
          >
            ×
          </button>
        </div>
        <div class="modalContent changesContent">
          <div v-if="isLoadingChanges" class="emptyState">
            Loading changes...
          </div>
          <div v-else-if="batchChanges.length === 0" class="emptyState">
            No changes in this batch.
          </div>
          <div v-else class="changesList">
            <div
              v-for="change in batchChanges"
              :key="change.id"
              class="changeItem"
            >
              <div class="changeHeader">
                <span
                  class="changeType"
                  :class="operationClass(change.operation)"
                >
                  {{ change.operation }}
                </span>
                <span class="changeEntity">{{ change.entity_type }}</span>
                <span class="changeTime">{{
                  formatDate(change.created_at)
                }}</span>
              </div>
              <div class="changeSummary">{{ change.display_summary }}</div>
              <div v-if="change.field_changes" class="changeDetails">
                <details>
                  <summary>Field changes</summary>
                  <pre class="fieldChanges">{{
                    formatFieldChanges(change.field_changes)
                  }}</pre>
                </details>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, reactive, computed, onMounted } from "vue";
import { useRemoteStore } from "@/store/remote";

const remoteStore = useRemoteStore();

// Tab and data state
const activeTab = ref("open");
const isLoading = ref(false);
const loadError = ref(null);

const batches = ref([]);
const closingBatches = reactive({});
const deletingBatches = reactive({});

// Create modal state
const showCreateModal = ref(false);
const createForm = reactive({
  name: "",
  description: "",
});
const isCreating = ref(false);
const createError = ref(null);

// Close modal state
const showCloseModal = ref(false);
const batchToClose = ref(null);
const isClosing = ref(false);
const closeError = ref(null);

// Delete modal state
const showDeleteModal = ref(false);
const batchToDelete = ref(null);
const isDeleting = ref(false);
const deleteError = ref(null);

// Changes modal state
const showChangesModal = ref(false);
const viewingBatch = ref(null);
const batchChanges = ref([]);
const isLoadingChanges = ref(false);

const openBatches = computed(() => batches.value.filter((b) => b.is_open));
const closedBatches = computed(() => batches.value.filter((b) => !b.is_open));

const tabs = computed(() => [
  { id: "open", label: "Open", count: openBatches.value.length },
  { id: "closed", label: "Closed", count: closedBatches.value.length },
]);

const loadData = async () => {
  isLoading.value = true;
  loadError.value = null;

  try {
    const result = await remoteStore.fetchChangelogBatches();
    if (result) {
      // API returns array directly, not { batches: [...] }
      batches.value = Array.isArray(result) ? result : [];
    } else {
      loadError.value = "Failed to load batches.";
    }
  } catch {
    loadError.value = "Failed to load batches.";
  }

  isLoading.value = false;
};

// Create batch
const openCreateModal = () => {
  createForm.name = "";
  createForm.description = "";
  createError.value = null;
  showCreateModal.value = true;
};

const closeCreateModal = () => {
  showCreateModal.value = false;
};

const submitCreateBatch = async () => {
  isCreating.value = true;
  createError.value = null;

  const result = await remoteStore.createChangelogBatch(
    createForm.name,
    createForm.description || null,
  );

  isCreating.value = false;

  if (result.error) {
    createError.value = result.error;
  } else {
    closeCreateModal();
    await loadData();
  }
};

// Close batch
const confirmCloseBatch = (batch) => {
  batchToClose.value = batch;
  closeError.value = null;
  showCloseModal.value = true;
};

const closeCloseModal = () => {
  showCloseModal.value = false;
  batchToClose.value = null;
};

const executeCloseBatch = async () => {
  if (!batchToClose.value) return;

  const batchId = batchToClose.value.id;
  isClosing.value = true;
  closingBatches[batchId] = true;
  closeError.value = null;

  const result = await remoteStore.closeChangelogBatch(batchId);

  if (result.error) {
    closeError.value = result.error;
    isClosing.value = false;
    closingBatches[batchId] = false;
  } else {
    closeCloseModal();
    await loadData();
    closingBatches[batchId] = false;
    isClosing.value = false;
  }
};

// Delete batch
const confirmDeleteBatch = (batch) => {
  batchToDelete.value = batch;
  deleteError.value = null;
  showDeleteModal.value = true;
};

const closeDeleteModal = () => {
  showDeleteModal.value = false;
  batchToDelete.value = null;
};

const executeDeleteBatch = async () => {
  if (!batchToDelete.value) return;

  const batchId = batchToDelete.value.id;
  isDeleting.value = true;
  deletingBatches[batchId] = true;
  deleteError.value = null;

  const result = await remoteStore.deleteChangelogBatch(batchId);

  if (result.error) {
    deleteError.value = result.error;
    isDeleting.value = false;
    deletingBatches[batchId] = false;
  } else {
    closeDeleteModal();
    await loadData();
    deletingBatches[batchId] = false;
    isDeleting.value = false;
  }
};

// View changes
const viewBatchChanges = async (batch) => {
  viewingBatch.value = batch;
  batchChanges.value = [];
  isLoadingChanges.value = true;
  showChangesModal.value = true;

  const result = await remoteStore.fetchChangelogBatchChanges(batch.id);

  if (result) {
    // API returns array directly, not { changes: [...] }
    batchChanges.value = Array.isArray(result) ? result : [];
  }

  isLoadingChanges.value = false;
};

const closeChangesModal = () => {
  showChangesModal.value = false;
  viewingBatch.value = null;
  batchChanges.value = [];
};

// Formatters
const formatDate = (timestamp) => {
  if (!timestamp) return "—";
  const date = new Date(timestamp * 1000);
  return date.toLocaleString();
};

const operationClass = (operation) => {
  switch (operation?.toLowerCase()) {
    case "create":
      return "operation-create";
    case "update":
      return "operation-update";
    case "delete":
      return "operation-delete";
    default:
      return "";
  }
};

const formatFieldChanges = (fieldChanges) => {
  if (!fieldChanges) return "";
  try {
    const parsed =
      typeof fieldChanges === "string"
        ? JSON.parse(fieldChanges)
        : fieldChanges;
    return JSON.stringify(parsed, null, 2);
  } catch {
    return fieldChanges;
  }
};

onMounted(() => {
  loadData();
});
</script>

<style scoped>
.batchManager {
  width: 100%;
  min-width: 0;
  color: var(--text-base);
}
.pageHeader {
  margin-bottom: 28px;
}
.sectionTitle {
  margin: 0 0 10px;
  font-size: 32px;
  font-weight: 700;
  line-height: 1.2;
  letter-spacing: -0.03em;
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
summary:focus-visible {
  outline: 2px solid #fff;
  outline-offset: 3px;
}
.actionButtons {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
  margin-bottom: 24px;
}
.actionButton,
.confirmButton {
  min-height: 44px;
  padding: 0 24px;
  border: 0;
  border-radius: 999px;
  background: var(--spotify-green);
  color: #000;
  font-weight: 700;
}
.actionButton:hover:not(:disabled),
.confirmButton:hover:not(:disabled) {
  background: var(--spotify-green-hover);
}
.refreshButton,
.viewButton,
.closeButton,
.deleteButton {
  min-height: 36px;
  padding: 6px 16px;
  border: 1px solid #727272;
  border-radius: 999px;
  background: transparent;
  color: var(--text-base);
  font-weight: 600;
  white-space: nowrap;
}
.refreshButton {
  margin-left: auto;
  min-height: 40px;
}
.refreshButton:hover:not(:disabled),
.viewButton:hover,
.closeButton:hover:not(:disabled),
.deleteButton:hover:not(:disabled) {
  border-color: #fff;
  background: #ffffff0c;
}
.deleteButton {
  color: #f3727f;
}
.statsSummary {
  display: flex;
  flex-wrap: wrap;
  gap: 12px 28px;
  padding-bottom: 24px;
  border-bottom: 1px solid var(--surface-border);
  margin-bottom: 24px;
  color: var(--text-subdued);
  font-size: 14px;
}
.statItem strong {
  color: var(--text-base);
  font-weight: 600;
  font-variant-numeric: tabular-nums;
}
.tabNav {
  display: flex;
  gap: 8px;
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
.batchList {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.batchItem {
  padding: 20px;
  border-radius: 8px;
  background: #181818;
}
.batchItemHeader,
.batchItemMain,
.batchItemActions {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 12px;
}
.batchItemHeader {
  justify-content: space-between;
  gap: 16px 24px;
}
.batchItemMain {
  flex: 1 1 240px;
  min-width: 0;
}
.batchItemName {
  font-size: 16px;
  font-weight: 500;
  overflow-wrap: anywhere;
}
.batchDescription {
  color: var(--text-subdued);
  font-size: 14px;
  line-height: 1.6;
  margin-top: 12px;
  overflow-wrap: anywhere;
}
.batchItemDetails {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 24px;
  margin-top: 14px;
  font-size: 12px;
}
.detailItem {
  display: inline-flex;
  flex-wrap: wrap;
  gap: 4px;
}
.detailLabel {
  color: var(--text-subdued);
}
.statusBadge,
.changeType {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  color: var(--text-subdued);
  font-size: 12px;
  text-transform: capitalize;
}
.statusBadge::before,
.changeType::before {
  content: "";
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: currentColor;
}
.statusBadge.status-open,
.operation-create {
  color: var(--spotify-green);
}
.operation-update {
  color: #f0bc65;
}
.operation-delete {
  color: #f3727f;
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
  text-align: center;
}
.errorMessage,
.modalError {
  padding: 12px 16px;
  background: #f3727f12;
  color: #f3727f;
  border-radius: 4px;
  margin-bottom: 20px;
  font-size: 14px;
  overflow-wrap: anywhere;
}
.detailOverlay {
  position: fixed;
  inset: 0;
  padding: 16px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #000b;
  z-index: 1000;
}
.detailPanel {
  display: flex;
  flex-direction: column;
  width: 480px;
  max-width: 100%;
  max-height: calc(100dvh - 32px);
  background: #282828;
  border-radius: 8px;
  box-shadow: var(--shadow-menu);
}
.changesModal {
  width: 760px;
}
.detailHeader {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-shrink: 0;
  gap: 16px;
  padding: 24px 24px 16px;
}
.detailTitle {
  margin: 0;
  font-size: 24px;
  font-weight: 700;
  letter-spacing: -0.02em;
  overflow-wrap: anywhere;
}
.closeDetailButton {
  display: grid;
  place-items: center;
  width: 36px;
  height: 36px;
  flex-shrink: 0;
  padding: 0;
  border: 0;
  border-radius: 50%;
  background: transparent;
  color: var(--text-subdued);
  font-size: 28px;
  line-height: 1;
}
.closeDetailButton:hover {
  background: #ffffff12;
  color: #fff;
}
.modalContent {
  min-height: 0;
  overflow-y: auto;
  overscroll-behavior: contain;
  padding: 8px 24px 24px;
}
.formGroup {
  margin-bottom: 20px;
}
.formLabel {
  display: block;
  margin-bottom: 8px;
  color: var(--text-subdued);
  font-size: 14px;
}
.formInput {
  width: 100%;
  min-width: 0;
  min-height: 44px;
  padding: 10px 12px;
  border: 1px solid #727272;
  border-radius: 4px;
  background: #333;
  color: var(--text-base);
  font: inherit;
  font-size: 14px;
}
.formInput:focus-visible {
  outline: 2px solid #fff;
  outline-offset: -2px;
}
.formInput::placeholder {
  color: var(--text-subdued);
}
.formTextarea {
  min-height: 100px;
  resize: vertical;
}
.modalActions {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 12px;
  margin-top: 24px;
}
.cancelButton,
.deleteConfirmButton {
  min-height: 44px;
  padding: 0 24px;
  border: 0;
  border-radius: 999px;
  font-weight: 700;
}
.cancelButton {
  background: transparent;
  color: var(--text-subdued);
}
.cancelButton:hover {
  color: #fff;
}
.deleteConfirmButton {
  background: #f3727f;
  color: #000;
}
.deleteConfirmButton:hover:not(:disabled) {
  background: #ff8e99;
}
.closeWarning,
.deleteWarning {
  color: var(--text-subdued);
  font-size: 14px;
  line-height: 1.6;
  margin: 0 0 20px;
}
.batchInfo {
  padding: 16px;
  background: #181818;
  border-radius: 4px;
}
.changesList {
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.changeItem {
  min-width: 0;
  padding: 16px;
  background: #181818;
  border-radius: 4px;
}
.changeHeader {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px 16px;
  margin-bottom: 12px;
}
.changeEntity,
.changeTime {
  color: var(--text-subdued);
  font-size: 12px;
}
.changeEntity {
  text-transform: capitalize;
}
.changeTime {
  margin-left: auto;
}
.changeSummary {
  font-size: 14px;
  line-height: 1.5;
  overflow-wrap: anywhere;
}
.changeDetails {
  margin-top: 16px;
}
.changeDetails summary {
  font-size: 13px;
  color: var(--text-subdued);
  cursor: pointer;
}
.fieldChanges {
  font-size: 12px;
  line-height: 1.5;
  background: #242424;
  padding: 12px;
  border-radius: 4px;
  color: var(--text-subdued);
  overflow-x: auto;
  margin: 12px 0 0;
}
@media (max-width: 600px) {
  .batchItem {
    padding: 16px;
  }
  .batchItemMain {
    flex-basis: 100%;
  }
  .batchItemActions {
    gap: 8px;
  }
  .detailHeader {
    padding: 20px 16px 12px;
  }
  .modalContent {
    padding: 8px 16px 20px;
  }
  .detailTitle {
    font-size: 22px;
  }
  .changeTime {
    width: 100%;
    margin-left: 0;
  }
}
</style>
