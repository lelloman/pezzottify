<template>
  <div class="playbackSessions">
    <div class="headerRow">
      <h2 class="sectionTitle">Playback Sessions</h2>
      <span class="autoRefreshBadge">Auto-refreshing every 5s</span>
    </div>

    <div v-if="isLoading && !sessions" class="loadingState">Loading...</div>

    <div v-else-if="loadError" class="errorMessage">{{ loadError }}</div>

    <div v-else-if="groupedSessions.length === 0" class="emptyState">
      No active playback sessions.
    </div>

    <div v-else class="sessionCards">
      <div
        v-for="group in groupedSessions"
        :key="group.user_handle"
        class="userCard"
      >
        <div class="userCardHeader">{{ group.user_handle }}</div>
        <div class="deviceList">
          <div
            v-for="device in group.devices"
            :key="device.device_id"
            class="deviceRow"
          >
            <div class="deviceHeader">
              <span
                class="statusDot"
                :class="device.is_playing ? 'dot-playing' : 'dot-paused'"
              ></span>
              <span class="deviceName">{{ device.device_name }}</span>
              <span class="typeBadge">{{ device.device_type }}</span>
            </div>
            <div v-if="device.track_title" class="trackInfo">
              <span class="trackTitle">{{ device.track_title }}</span>
              <span v-if="device.artist_name" class="trackArtist">
                — {{ device.artist_name }}
              </span>
              <span v-if="!device.is_playing" class="pausedLabel">
                (paused)
              </span>
            </div>
            <div v-else class="trackInfo trackInfoEmpty">No track loaded</div>
            <div class="deviceMeta">
              <span class="position">
                {{ formatTime(device.position) }}
              </span>
              <span class="separator">·</span>
              <span class="queueLength">
                {{ device.queue_length }} in queue
              </span>
              <span class="separator">·</span>
              <span class="staleness" :class="stalenessClass(device)">
                updated {{ formatStaleness(device.last_update_secs_ago) }}
              </span>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted } from "vue";
import { useRemoteStore } from "@/store/remote";

const remoteStore = useRemoteStore();

const sessions = ref(null);
const isLoading = ref(false);
const loadError = ref(null);

const POLL_INTERVAL = 5000;
let pollTimer = null;

const groupedSessions = computed(() => {
  if (!sessions.value || !Array.isArray(sessions.value)) return [];

  // API already returns grouped: [{ user_id, user_handle, devices: [...] }]
  return [...sessions.value].sort((a, b) =>
    (a.user_handle || "").localeCompare(b.user_handle || ""),
  );
});

const fetchData = async () => {
  isLoading.value = true;
  loadError.value = null;
  const result = await remoteStore.fetchPlaybackSessions();
  if (result !== null) {
    sessions.value = result;
  } else if (sessions.value === null) {
    loadError.value = "Failed to load playback sessions.";
  }
  isLoading.value = false;
};

const formatTime = (secs) => {
  if (secs == null) return "0:00";
  const s = Math.floor(secs);
  const m = Math.floor(s / 60);
  const sec = s % 60;
  return `${m}:${sec.toString().padStart(2, "0")}`;
};

const formatStaleness = (secs) => {
  if (secs == null) return "just now";
  if (secs < 60) return `${Math.floor(secs)}s ago`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m ago`;
  return `${Math.floor(secs / 3600)}h ago`;
};

const stalenessClass = (device) => {
  const secs = device.last_update_secs_ago;
  if (secs == null) return "";
  if (secs > 60) return "staleness-stale";
  if (secs > 30) return "staleness-warning";
  return "";
};

onMounted(() => {
  fetchData();
  pollTimer = setInterval(fetchData, POLL_INTERVAL);
});

onUnmounted(() => {
  if (pollTimer) {
    clearInterval(pollTimer);
    pollTimer = null;
  }
});
</script>

<style scoped>
.playbackSessions {
  width: 100%;
}

.headerRow {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--spacing-6);
}

.sectionTitle {
  font-size: var(--text-2xl);
  font-weight: var(--font-bold);
  color: var(--text-base);
  margin: 0;
}

.autoRefreshBadge {
  font-size: var(--text-sm);
  color: var(--text-subdued);
}

.loadingState,
.emptyState {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--spacing-8);
  color: var(--text-subdued);
}

.errorMessage {
  padding: var(--spacing-3) var(--spacing-4);
  background-color: rgba(220, 38, 38, 0.1);
  border: 1px solid #dc2626;
  border-radius: var(--radius-md);
  color: #dc2626;
  font-size: var(--text-sm);
  margin-bottom: var(--spacing-4);
}

.sessionCards {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-4);
}

.userCard {
  background-color: var(--bg-elevated-base);
  border-radius: var(--radius-lg);
  overflow: hidden;
}

.userCardHeader {
  padding: var(--spacing-3) var(--spacing-4);
  font-size: var(--text-base);
  font-weight: var(--font-semibold);
  color: var(--text-base);
  border-bottom: 1px solid var(--border-subdued);
}

.deviceList {
  display: flex;
  flex-direction: column;
}

.deviceRow {
  padding: var(--spacing-3) var(--spacing-4);
}

.deviceRow + .deviceRow {
  border-top: 1px solid var(--border-subdued);
}

.deviceHeader {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  margin-bottom: var(--spacing-1);
}

.statusDot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.dot-playing {
  background-color: #22c55e;
  box-shadow: 0 0 4px rgba(34, 197, 94, 0.5);
}

.dot-paused {
  background-color: #6b7280;
}

.deviceName {
  font-size: var(--text-sm);
  font-weight: var(--font-medium);
  color: var(--text-base);
}

.typeBadge {
  font-size: var(--text-xs);
  padding: 1px var(--spacing-2);
  background-color: rgba(255, 255, 255, 0.1);
  border-radius: var(--radius-full);
  color: var(--text-subdued);
}

.trackInfo {
  font-size: var(--text-sm);
  margin-bottom: var(--spacing-1);
  padding-left: calc(8px + var(--spacing-2));
}

.trackTitle {
  color: var(--text-base);
}

.trackArtist {
  color: var(--text-subdued);
}

.pausedLabel {
  color: var(--text-subdued);
  font-style: italic;
}

.trackInfoEmpty {
  color: var(--text-subdued);
  font-style: italic;
}

.deviceMeta {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  font-size: var(--text-xs);
  color: var(--text-subdued);
  padding-left: calc(8px + var(--spacing-2));
}

.separator {
  opacity: 0.5;
}

.staleness-warning {
  color: #f59e0b;
}

.staleness-stale {
  color: #ef4444;
}
</style>
