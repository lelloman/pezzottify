<template>
  <div v-if="shouldShow" class="enrichmentStatusIndicator">
    <button
      type="button"
      class="enrichmentStatusButton"
      :class="statusClass"
      :title="buttonTitle"
      @click="showDialog = true"
    >
      <svg
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
      >
        <circle cx="12" cy="12" r="10" />
        <path d="M12 16v-4" />
        <path d="M12 8h.01" />
      </svg>
    </button>
    <ModalDialog
      :isOpen="showDialog"
      :closeCallback="() => (showDialog = false)"
      :closeOnEsc="true"
    >
      <div class="enrichmentDialog" role="dialog" aria-modal="true">
        <div class="enrichmentDialogHeader">
          <h2>{{ dialogTitle }}</h2>
          <button
            type="button"
            class="enrichmentDialogClose"
            title="Close"
            @click="showDialog = false"
          >
            x
          </button>
        </div>
        <p>{{ dialogMessage }}</p>
        <dl class="enrichmentDetails">
          <div>
            <dt>Status</dt>
            <dd>{{ statusLabel }}</dd>
          </div>
          <div v-if="stage">
            <dt>Stage</dt>
            <dd>{{ stage }}</dd>
          </div>
          <div v-if="attempts !== null">
            <dt>Attempts</dt>
            <dd>{{ attempts }}</dd>
          </div>
          <div v-if="lastError">
            <dt>Last error</dt>
            <dd>{{ lastError }}</dd>
          </div>
        </dl>
      </div>
    </ModalDialog>
  </div>
</template>

<script setup>
import { computed, ref } from "vue";
import ModalDialog from "@/components/common/ModalDialog.vue";

const props = defineProps({
  status: {
    type: Object,
    default: null,
  },
  entityType: {
    type: String,
    required: true,
  },
});

const showDialog = ref(false);

const normalizedStatus = computed(() => props.status?.status || null);
const shouldShow = computed(() =>
  ["queued", "running", "failed"].includes(normalizedStatus.value),
);

const entityLabel = computed(() => props.entityType || "item");
const statusLabel = computed(() => {
  if (normalizedStatus.value === "queued") return "Queued";
  if (normalizedStatus.value === "running") return "Running";
  if (normalizedStatus.value === "failed") return "Failed";
  return "Unknown";
});

const statusClass = computed(() => `is-${normalizedStatus.value}`);
const buttonTitle = computed(
  () => `${entityLabel.value} enrichment: ${statusLabel.value}`,
);
const dialogTitle = computed(() => `${entityLabel.value} enrichment`);

const dialogMessage = computed(() => {
  if (normalizedStatus.value === "queued") {
    return "Pezzottify is waiting to generate richer metadata for this item. The page remains usable, and the details will appear here after the background job completes.";
  }
  if (normalizedStatus.value === "running") {
    return "Pezzottify is currently generating richer metadata for this item. Refresh this page later to see the updated details.";
  }
  if (normalizedStatus.value === "failed") {
    return "Pezzottify tried to generate richer metadata for this item but the job failed. It may retry later, or an administrator can inspect the enrichment job.";
  }
  return "Pezzottify can generate richer metadata for catalog items in the background.";
});

const stage = computed(() => props.status?.stage || null);
const attempts = computed(() => props.status?.attempts ?? null);
const lastError = computed(() => props.status?.last_error || null);
</script>

<style scoped>
.enrichmentStatusIndicator {
  position: absolute;
  right: 14px;
  bottom: 14px;
  z-index: 2;
}

.enrichmentStatusButton {
  display: inline-flex;
  width: 34px;
  height: 34px;
  align-items: center;
  justify-content: center;
  border: 1px solid var(--surface-border-strong);
  border-radius: 999px;
  background: rgba(0, 0, 0, 0.32);
  color: var(--text-muted);
  cursor: pointer;
  transition:
    background-color var(--transition-fast),
    border-color var(--transition-fast),
    color var(--transition-fast);
}

.enrichmentStatusButton svg {
  width: 18px;
  height: 18px;
}

.enrichmentStatusButton:hover {
  background: var(--surface-hover);
  color: var(--text-base);
}

.enrichmentStatusButton.is-running {
  color: var(--spotify-green);
}

.enrichmentStatusButton.is-failed {
  color: #ff8f8f;
}

.enrichmentDialog {
  width: min(420px, calc(100vw - 48px));
  color: var(--text-base);
}

.enrichmentDialogHeader {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 12px;
}

.enrichmentDialogHeader h2 {
  margin: 0;
  font-size: 1.05rem;
  font-weight: 850;
}

.enrichmentDialogClose {
  width: 32px;
  height: 32px;
  border: 1px solid var(--surface-border);
  border-radius: 999px;
  background: transparent;
  color: var(--text-muted);
  cursor: pointer;
  font-size: 1.25rem;
  line-height: 1;
}

.enrichmentDialogClose:hover {
  color: var(--text-base);
  background: var(--surface-hover);
}

.enrichmentDialog p {
  margin: 0;
  color: var(--text-muted);
  font-size: 0.95rem;
  line-height: 1.5;
}

.enrichmentDetails {
  margin: 16px 0 0;
}

.enrichmentDetails div {
  display: grid;
  grid-template-columns: 88px minmax(0, 1fr);
  gap: 12px;
  padding: 8px 0;
  border-top: 1px solid var(--surface-border);
}

.enrichmentDetails dt {
  color: var(--text-subdued);
  font-size: 0.78rem;
  font-weight: 800;
  text-transform: uppercase;
}

.enrichmentDetails dd {
  min-width: 0;
  margin: 0;
  color: var(--text-base);
  font-size: 0.9rem;
  overflow-wrap: anywhere;
}
</style>
