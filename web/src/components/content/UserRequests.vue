<template>
  <div class="requests-container">
    <h1 class="page-title">My Requests</h1>

    <!-- Rate Limits Section -->
    <div v-if="limits" class="limits-section">
      <div class="limit-card">
        <span class="limit-label">Today's Requests</span>
        <span class="limit-value" :class="{ warning: limits.requests_today >= limits.max_per_day }">
          {{ limits.requests_today }} / {{ limits.max_per_day }}
        </span>
      </div>
      <div class="limit-card">
        <span class="limit-label">In Queue</span>
        <span class="limit-value" :class="{ warning: limits.in_queue >= limits.max_queue }">
          {{ limits.in_queue }} / {{ limits.max_queue }}
        </span>
      </div>
      <div class="limit-card">
        <span class="limit-label">Status</span>
        <span class="limit-value" :class="limits.can_request ? 'can-request' : 'cannot-request'">
          {{ limits.can_request ? 'Can Request' : 'At Limit' }}
        </span>
      </div>
    </div>

    <!-- Pending Requests Section -->
    <div class="section">
      <h2 class="section-title">Pending</h2>
      <div v-if="pendingRequests.length > 0" class="requests-list">
        <div v-for="request in pendingRequests" :key="request.id" class="request-card">
          <div class="request-info">
            <span class="request-name">{{ request.content_name }}</span>
            <span v-if="request.artist_name" class="request-artist">{{ request.artist_name }}</span>
            <span class="request-meta">
              <span class="status-badge" :class="getStatusClass(request.status)">
                {{ formatStatus(request.status) }}
              </span>
              <span v-if="request.queue_position" class="queue-position">
                #{{ request.queue_position }} in queue
              </span>
            </span>
          </div>
          <div v-if="request.progress" class="progress-bar">
            <div
              class="progress-fill"
              :style="{ width: getProgressPercent(request.progress) + '%' }"
            ></div>
            <span class="progress-text">
              {{ request.progress.completed }}/{{ request.progress.total_children }}
            </span>
          </div>
        </div>
      </div>
      <p v-else class="empty-message">No pending requests</p>
    </div>

    <!-- Completed Requests Section -->
    <div class="section">
      <h2 class="section-title">Completed</h2>
      <div v-if="completedRequests.length > 0" class="requests-list">
        <div v-for="request in completedRequests" :key="request.id" class="request-card completed">
          <div class="request-info">
            <router-link
              v-if="request.status === 'COMPLETED' && request.content_type === 'ALBUM'"
              :to="`/album/${request.content_id}`"
              class="request-name request-link"
            >
              {{ request.content_name }}
            </router-link>
            <span v-else class="request-name">{{ request.content_name }}</span>
            <span v-if="request.artist_name" class="request-artist">{{ request.artist_name }}</span>
            <span class="request-meta">
              <span class="status-badge" :class="getStatusClass(request.status)">
                {{ formatStatus(request.status) }}
              </span>
              <span v-if="request.completed_at" class="completed-date">
                {{ formatDate(request.completed_at) }}
              </span>
            </span>
            <span v-if="request.error_message" class="error-message">
              {{ request.error_message }}
            </span>
          </div>
        </div>
      </div>
      <p v-else class="empty-message">No completed requests</p>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted } from "vue";

const limits = ref(null);
const requests = ref([]);
const isLoading = ref(true);

const pendingRequests = computed(() => {
  return requests.value.filter(
    (r) => !["COMPLETED", "FAILED"].includes(r.status)
  );
});

const completedRequests = computed(() => {
  return requests.value.filter((r) =>
    ["COMPLETED", "FAILED"].includes(r.status)
  );
});

const fetchLimits = async () => {
  try {
    const response = await fetch("/v1/download/limits");
    if (response.ok) {
      limits.value = await response.json();
    }
  } catch (error) {
    console.error("Error fetching limits:", error);
  }
};

const fetchRequests = async () => {
  try {
    const response = await fetch("/v1/download/my-requests");
    if (response.ok) {
      requests.value = await response.json();
    }
  } catch (error) {
    console.error("Error fetching requests:", error);
  }
};

const formatStatus = (status) => {
  const statusMap = {
    PENDING: "Pending",
    IN_PROGRESS: "Downloading",
    RETRY_WAITING: "Retrying",
    COMPLETED: "Completed",
    FAILED: "Failed",
  };
  return statusMap[status] || status;
};

const getStatusClass = (status) => {
  return {
    pending: status === "PENDING",
    "in-progress": status === "IN_PROGRESS",
    "retry-waiting": status === "RETRY_WAITING",
    completed: status === "COMPLETED",
    failed: status === "FAILED",
  };
};

const getProgressPercent = (progress) => {
  if (!progress || progress.total_children === 0) return 0;
  return Math.round(
    ((progress.completed + progress.failed) / progress.total_children) * 100
  );
};

const formatDate = (timestamp) => {
  const date = new Date(timestamp * 1000);
  return date.toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
};

onMounted(async () => {
  isLoading.value = true;
  await Promise.all([fetchLimits(), fetchRequests()]);
  isLoading.value = false;
});
</script>

<style scoped>
.requests-container {
  max-width: 800px;
  margin: 0 auto;
  padding: var(--spacing-4);
}

.page-title {
  font-size: var(--text-2xl);
  font-weight: var(--font-bold);
  color: var(--text-base);
  margin-bottom: var(--spacing-6);
}

/* Limits Section */
.limits-section {
  display: flex;
  gap: var(--spacing-4);
  margin-bottom: var(--spacing-6);
  flex-wrap: wrap;
}

.limit-card {
  flex: 1;
  min-width: 150px;
  background-color: var(--bg-elevated);
  border-radius: var(--radius-md);
  padding: var(--spacing-4);
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
}

.limit-label {
  font-size: var(--text-sm);
  color: var(--text-subdued);
}

.limit-value {
  font-size: var(--text-xl);
  font-weight: var(--font-semibold);
  color: var(--text-base);
}

.limit-value.warning {
  color: #ef4444;
}

.limit-value.can-request {
  color: #22c55e;
}

.limit-value.cannot-request {
  color: #ef4444;
}

/* Section */
.section {
  margin-bottom: var(--spacing-6);
}

.section-title {
  font-size: var(--text-lg);
  font-weight: var(--font-semibold);
  color: var(--text-base);
  margin-bottom: var(--spacing-3);
  padding-bottom: var(--spacing-2);
  border-bottom: 1px solid var(--border-subdued);
}

/* Requests List */
.requests-list {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-3);
}

.request-card {
  background-color: var(--bg-elevated);
  border-radius: var(--radius-md);
  padding: var(--spacing-4);
}

.request-card.completed {
  opacity: 0.8;
}

.request-info {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-1);
}

.request-name {
  font-weight: var(--font-medium);
  color: var(--text-base);
}

.request-link {
  color: var(--text-base);
  text-decoration: none;
}

.request-link:hover {
  color: var(--spotify-green);
  text-decoration: underline;
}

.request-artist {
  font-size: var(--text-sm);
  color: var(--text-subdued);
}

.request-meta {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  margin-top: var(--spacing-1);
}

.status-badge {
  font-size: var(--text-xs);
  padding: 2px 8px;
  border-radius: var(--radius-full);
  font-weight: var(--font-medium);
}

.status-badge.pending {
  background-color: rgba(107, 114, 128, 0.2);
  color: #9ca3af;
}

.status-badge.in-progress {
  background-color: rgba(59, 130, 246, 0.2);
  color: #3b82f6;
}

.status-badge.retry-waiting {
  background-color: rgba(249, 115, 22, 0.2);
  color: #f97316;
}

.status-badge.completed {
  background-color: rgba(34, 197, 94, 0.2);
  color: #22c55e;
}

.status-badge.failed {
  background-color: rgba(239, 68, 68, 0.2);
  color: #ef4444;
}

.queue-position {
  font-size: var(--text-xs);
  color: var(--text-subdued);
}

.completed-date {
  font-size: var(--text-xs);
  color: var(--text-subdued);
}

.error-message {
  font-size: var(--text-sm);
  color: #ef4444;
  margin-top: var(--spacing-2);
}

/* Progress Bar */
.progress-bar {
  margin-top: var(--spacing-3);
  height: 8px;
  background-color: var(--bg-subdued);
  border-radius: var(--radius-full);
  position: relative;
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  background-color: var(--spotify-green);
  border-radius: var(--radius-full);
  transition: width 0.3s ease;
}

.progress-text {
  position: absolute;
  right: 0;
  top: -20px;
  font-size: var(--text-xs);
  color: var(--text-subdued);
}

.empty-message {
  color: var(--text-subdued);
  font-style: italic;
}
</style>
