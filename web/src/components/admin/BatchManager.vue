<template>
  <div class="batchManager">
    <h2 class="sectionTitle">Catalog Batches</h2>

    <!-- Action Buttons -->
    <div class="actionButtons">
      <button class="actionButton" @click="openCreateModal">
        New Batch
      </button>
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
        @click="activeTab = tab.id"
      >
        {{ tab.label }}
        <span v-if="tab.count !== undefined" class="tabCount">{{ tab.count }}</span>
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
        <div v-for="batch in openBatches" :key="batch.id" class="batchItem status-open">
          <div class="batchItemHeader">
            <div class="batchItemMain">
              <span class="batchItemName">{{ batch.name }}</span>
              <span class="statusBadge status-open">open</span>
            </div>
            <div class="batchItemActions">
              <button
                class="viewButton"
                @click="viewBatchChanges(batch)"
              >
                View Changes
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
              <span class="detailValue">{{ formatDate(batch.created_at) }}</span>
            </span>
            <span class="detailItem">
              <span class="detailLabel">Last activity:</span>
              <span class="detailValue">{{ formatDate(batch.last_activity_at) }}</span>
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
        <div v-for="batch in closedBatches" :key="batch.id" class="batchItem status-closed">
          <div class="batchItemHeader">
            <div class="batchItemMain">
              <span class="batchItemName">{{ batch.name }}</span>
              <span class="statusBadge status-closed">closed</span>
            </div>
            <div class="batchItemActions">
              <button
                class="viewButton"
                @click="viewBatchChanges(batch)"
              >
                View Changes
              </button>
            </div>
          </div>
          <div v-if="batch.description" class="batchDescription">
            {{ batch.description }}
          </div>
          <div class="batchItemDetails">
            <span class="detailItem">
              <span class="detailLabel">Created:</span>
              <span class="detailValue">{{ formatDate(batch.created_at) }}</span>
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
    <div v-if="showCreateModal" class="detailOverlay" @click.self="closeCreateModal">
      <div class="detailPanel createModal">
        <div class="detailHeader">
          <h3 class="detailTitle">Create New Batch</h3>
          <button class="closeDetailButton" @click="closeCreateModal">×</button>
        </div>
        <div class="modalContent">
          <div class="formGroup">
            <label class="formLabel">Batch Name</label>
            <input
              v-model="createForm.name"
              type="text"
              class="formInput"
              placeholder="e.g., December 2024 Updates"
            />
          </div>
          <div class="formGroup">
            <label class="formLabel">Description (optional)</label>
            <textarea
              v-model="createForm.description"
              class="formInput formTextarea"
              placeholder="Brief description of the batch contents"
            ></textarea>
          </div>
          <div v-if="createError" class="modalError">
            {{ createError }}
          </div>
          <div class="modalActions">
            <button class="cancelButton" @click="closeCreateModal">Cancel</button>
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

    <!-- Close Batch Confirmation Modal -->
    <div v-if="showCloseModal" class="detailOverlay" @click.self="closeCloseModal">
      <div class="detailPanel closeModal">
        <div class="detailHeader">
          <h3 class="detailTitle">Close Batch</h3>
          <button class="closeDetailButton" @click="closeCloseModal">×</button>
        </div>
        <div class="modalContent">
          <p class="closeWarning">
            Are you sure you want to close this batch? Once closed, no more changes can be added to it.
          </p>
          <div class="batchInfo">
            <span class="batchItemName">{{ batchToClose?.name }}</span>
          </div>
          <div v-if="closeError" class="modalError">
            {{ closeError }}
          </div>
          <div class="modalActions">
            <button class="cancelButton" @click="closeCloseModal">Cancel</button>
            <button
              class="confirmButton"
              @click="executeCloseBatch"
              :disabled="isClosing"
            >
              {{ isClosing ? "Closing..." : "Close Batch" }}
            </button>
          </div>
        </div>
      </div>
    </div>

    <!-- Delete Batch Confirmation Modal -->
    <div v-if="showDeleteModal" class="detailOverlay" @click.self="closeDeleteModal">
      <div class="detailPanel deleteModal">
        <div class="detailHeader">
          <h3 class="detailTitle">Delete Batch</h3>
          <button class="closeDetailButton" @click="closeDeleteModal">×</button>
        </div>
        <div class="modalContent">
          <p class="deleteWarning">
            Are you sure you want to delete this batch? This action cannot be undone.
            Note: Only empty batches can be deleted.
          </p>
          <div class="batchInfo">
            <span class="batchItemName">{{ batchToDelete?.name }}</span>
          </div>
          <div v-if="deleteError" class="modalError">
            {{ deleteError }}
          </div>
          <div class="modalActions">
            <button class="cancelButton" @click="closeDeleteModal">Cancel</button>
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

    <!-- View Changes Modal -->
    <div v-if="showChangesModal" class="detailOverlay" @click.self="closeChangesModal">
      <div class="detailPanel changesModal">
        <div class="detailHeader">
          <h3 class="detailTitle">Batch Changes: {{ viewingBatch?.name }}</h3>
          <button class="closeDetailButton" @click="closeChangesModal">×</button>
        </div>
        <div class="modalContent changesContent">
          <div v-if="isLoadingChanges" class="emptyState">
            Loading changes...
          </div>
          <div v-else-if="batchChanges.length === 0" class="emptyState">
            No changes in this batch.
          </div>
          <div v-else class="changesList">
            <div v-for="change in batchChanges" :key="change.id" class="changeItem">
              <div class="changeHeader">
                <span class="changeType" :class="operationClass(change.operation)">
                  {{ change.operation }}
                </span>
                <span class="changeEntity">{{ change.entity_type }}</span>
                <span class="changeTime">{{ formatDate(change.created_at) }}</span>
              </div>
              <div class="changeSummary">{{ change.display_summary }}</div>
              <div v-if="change.field_changes" class="changeDetails">
                <details>
                  <summary>Field changes</summary>
                  <pre class="fieldChanges">{{ formatFieldChanges(change.field_changes) }}</pre>
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

const openBatches = computed(() => batches.value.filter(b => b.is_open));
const closedBatches = computed(() => batches.value.filter(b => !b.is_open));

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
    const parsed = typeof fieldChanges === "string" ? JSON.parse(fieldChanges) : fieldChanges;
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
}

.sectionTitle {
  font-size: var(--text-2xl);
  font-weight: var(--font-bold);
  color: var(--text-base);
  margin: 0 0 var(--spacing-4) 0;
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

/* Batch List */
.batchList {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
}

.batchItem {
  background-color: var(--bg-elevated-base);
  border-radius: var(--radius-md);
  padding: var(--spacing-3) var(--spacing-4);
  border-left: 3px solid var(--border-subdued);
}

.batchItem.status-open {
  border-left-color: #22c55e;
}

.batchItem.status-closed {
  border-left-color: #9ca3af;
}

.batchItemHeader {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: var(--spacing-3);
}

.batchItemMain {
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
}

.batchItemName {
  font-weight: var(--font-medium);
  color: var(--text-base);
}

.batchDescription {
  margin-top: var(--spacing-2);
  font-size: var(--text-sm);
  color: var(--text-subdued);
}

.batchItemActions {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  flex-shrink: 0;
}

.batchItemDetails {
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

/* Status Badge */
.statusBadge {
  display: inline-block;
  padding: 2px 8px;
  border-radius: var(--radius-full);
  font-size: var(--text-xs);
  font-weight: var(--font-medium);
}

.statusBadge.status-open {
  background-color: rgba(34, 197, 94, 0.15);
  color: #22c55e;
}

.statusBadge.status-closed {
  background-color: rgba(156, 163, 175, 0.15);
  color: #9ca3af;
}

/* Buttons */
.viewButton {
  padding: 2px 10px;
  background-color: var(--bg-highlight);
  color: var(--text-base);
  border: none;
  border-radius: var(--radius-md);
  font-size: var(--text-xs);
  cursor: pointer;
  transition: background-color var(--transition-fast);
}

.viewButton:hover {
  background-color: var(--bg-elevated-highlight);
}

.closeButton {
  padding: 2px 10px;
  background-color: var(--spotify-green);
  color: white;
  border: none;
  border-radius: var(--radius-md);
  font-size: var(--text-xs);
  cursor: pointer;
  transition: background-color var(--transition-fast);
}

.closeButton:hover:not(:disabled) {
  background-color: #1ed760;
}

.closeButton:disabled {
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

.changesModal {
  max-width: 700px;
  max-height: 80vh;
  display: flex;
  flex-direction: column;
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

.changesContent {
  overflow-y: auto;
  max-height: 60vh;
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

.formTextarea {
  min-height: 80px;
  resize: vertical;
  font-family: inherit;
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

.closeWarning,
.deleteWarning {
  color: var(--text-subdued);
  margin: 0 0 var(--spacing-4) 0;
}

.batchInfo {
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
  padding: var(--spacing-3);
  background-color: var(--bg-base);
  border-radius: var(--radius-md);
  margin-bottom: var(--spacing-4);
}

/* Changes List */
.changesList {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-3);
}

.changeItem {
  background-color: var(--bg-base);
  border-radius: var(--radius-md);
  padding: var(--spacing-3);
}

.changeHeader {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  margin-bottom: var(--spacing-2);
}

.changeType {
  padding: 2px 8px;
  border-radius: var(--radius-md);
  font-size: var(--text-xs);
  font-weight: var(--font-medium);
  text-transform: uppercase;
}

.operation-create {
  background-color: rgba(34, 197, 94, 0.15);
  color: #22c55e;
}

.operation-update {
  background-color: rgba(59, 130, 246, 0.15);
  color: #3b82f6;
}

.operation-delete {
  background-color: rgba(220, 38, 38, 0.15);
  color: #dc2626;
}

.changeEntity {
  font-size: var(--text-xs);
  color: var(--text-subdued);
  text-transform: capitalize;
}

.changeTime {
  font-size: var(--text-xs);
  color: var(--text-subdued);
  margin-left: auto;
}

.changeSummary {
  font-size: var(--text-sm);
  color: var(--text-base);
}

.changeDetails {
  margin-top: var(--spacing-2);
}

.changeDetails summary {
  font-size: var(--text-xs);
  color: var(--text-subdued);
  cursor: pointer;
}

.fieldChanges {
  font-size: var(--text-xs);
  color: var(--text-subdued);
  background-color: var(--bg-elevated-base);
  padding: var(--spacing-2);
  border-radius: var(--radius-md);
  overflow-x: auto;
  margin-top: var(--spacing-2);
}

@media (max-width: 768px) {
  .actionButtons {
    flex-wrap: wrap;
  }

  .refreshButton {
    margin-left: 0;
    width: 100%;
  }

  .batchItemHeader {
    flex-direction: column;
    gap: var(--spacing-2);
  }

  .batchItemActions {
    width: 100%;
    justify-content: flex-start;
  }
}
</style>
