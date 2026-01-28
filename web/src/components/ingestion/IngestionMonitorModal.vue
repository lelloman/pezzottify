<template>
  <ModalDialog
    :isOpen="ingestionStore.isModalOpen"
    :closeCallback="close"
    :closeOnEsc="true"
  >
    <div class="ingestion-monitor">
      <header class="monitor-header">
        <h2>Ingestion Monitor</h2>
        <button class="close-btn" @click="close" title="Close">&times;</button>
      </header>

      <IngestionTabs
        v-if="ingestionStore.visibleSessions.length > 1"
        :sessions="ingestionStore.visibleSessions"
        :activeTabId="ingestionStore.activeTabId"
        @select="ingestionStore.setActiveTab"
      />

      <div v-if="session" class="monitor-content">
        <IngestionStatusBar :job="session.job" :session="session" />

        <IngestionSection
          title="Upload Details"
          :badge="session.filesTotal + ' files'"
          :defaultOpen="true"
        >
          <UploadDetailsSection :job="session.job" :files="session.files" />
        </IngestionSection>

        <IngestionSection
          v-if="session.job?.matched_album_id || session.candidates?.length"
          title="Album Match"
          :badge="matchBadge"
        >
          <AlbumMatchSection
            :job="session.job"
            :candidates="session.candidates"
          />
        </IngestionSection>

        <IngestionSection
          v-if="showTrackMapping"
          title="Track Mapping"
          :badge="trackMappingBadge"
        >
          <TrackMappingSection :files="session.files" />
        </IngestionSection>

        <IngestionSection
          v-if="showConversion"
          title="Conversion"
          :badge="conversionBadge"
        >
          <ConversionSection :files="session.files" :job="session.job" />
        </IngestionSection>

        <ReviewSection
          v-if="session.review && session.job?.status === 'AWAITING_REVIEW'"
          :review="session.review"
          :jobId="session.job.id"
          @resolve="handleResolve"
        />
      </div>

      <div v-else class="no-sessions">
        <p>No active ingestion jobs</p>
      </div>

      <footer v-if="session" class="monitor-footer">
        <button
          v-if="isComplete"
          class="dismiss-btn"
          @click="dismiss"
        >
          Dismiss
        </button>
      </footer>
    </div>
  </ModalDialog>
</template>

<script setup>
import { computed } from "vue";
import { useIngestionStore } from "../../store/ingestion";
import ModalDialog from "../common/ModalDialog.vue";
import IngestionTabs from "./IngestionTabs.vue";
import IngestionStatusBar from "./IngestionStatusBar.vue";
import IngestionSection from "./IngestionSection.vue";
import UploadDetailsSection from "./sections/UploadDetailsSection.vue";
import AlbumMatchSection from "./sections/AlbumMatchSection.vue";
import TrackMappingSection from "./sections/TrackMappingSection.vue";
import ConversionSection from "./sections/ConversionSection.vue";
import ReviewSection from "./sections/ReviewSection.vue";

const ingestionStore = useIngestionStore();

const session = computed(() => ingestionStore.activeSession);

const isComplete = computed(() => {
  const status = session.value?.job?.status;
  return status === "COMPLETED" || status === "FAILED";
});

const showTrackMapping = computed(() => {
  const status = session.value?.job?.status;
  return ["MAPPING_TRACKS", "CONVERTING", "COMPLETED"].includes(status);
});

const showConversion = computed(() => {
  const status = session.value?.job?.status;
  return ["CONVERTING", "COMPLETED"].includes(status);
});

const matchBadge = computed(() => {
  const confidence = session.value?.job?.match_confidence;
  if (confidence) {
    return `${Math.round(confidence * 100)}%`;
  }
  return null;
});

const trackMappingBadge = computed(() => {
  const matched = session.value?.job?.tracks_matched || 0;
  const total = session.value?.filesTotal || 0;
  return `${matched}/${total}`;
});

const conversionBadge = computed(() => {
  const converted = session.value?.job?.tracks_converted || 0;
  const total = session.value?.filesTotal || 0;
  return `${converted}/${total}`;
});

function close() {
  ingestionStore.closeModal();
}

function dismiss() {
  if (session.value?.job?.id) {
    ingestionStore.dismissSession(session.value.job.id);
  }
}

async function handleResolve({ jobId, optionId }) {
  await ingestionStore.resolveReview(jobId, optionId);
}
</script>

<style scoped>
.ingestion-monitor {
  width: 600px;
  max-width: 90vw;
  max-height: 80vh;
  display: flex;
  flex-direction: column;
  background: var(--bg-elevated) !important;
  color: var(--text-base) !important;
  border-radius: 8px;
  padding: 20px;
}

.monitor-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding-bottom: 12px;
  border-bottom: 1px solid var(--border-default);
  margin-bottom: 12px;
}

.monitor-header h2 {
  margin: 0;
  font-size: 18px;
  font-weight: 600;
  color: var(--text-base);
}

.close-btn {
  background: none;
  border: none;
  font-size: 24px;
  cursor: pointer;
  color: var(--text-subdued);
  padding: 0;
  line-height: 1;
}

.close-btn:hover {
  color: var(--text-base);
}

.monitor-content {
  flex: 1;
  overflow-y: auto;
  padding-right: 4px;
}

.no-sessions {
  padding: 40px 20px;
  text-align: center;
  color: var(--text-subdued);
}

.monitor-footer {
  display: flex;
  justify-content: flex-end;
  padding-top: 12px;
  border-top: 1px solid var(--border-default);
  margin-top: 12px;
}

.dismiss-btn {
  padding: 8px 16px;
  background: var(--spotify-green);
  color: var(--text-negative);
  border: none;
  border-radius: 4px;
  cursor: pointer;
  font-size: 14px;
}

.dismiss-btn:hover {
  filter: brightness(1.1);
}
</style>
