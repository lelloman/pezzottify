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
        <span class="limit-label">Status</span>
        <span class="limit-value" :class="limits.can_request ? 'can-request' : 'cannot-request'">
          {{ limits.can_request ? 'Can Request' : 'At Limit' }}
        </span>
      </div>
    </div>

    <!-- Pending Requests Section -->
    <div class="section">
      <h2 class="section-title">Pending ({{ pendingRequests.length }})</h2>
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
      <h2 class="section-title">Completed ({{ completedRequests.length }})</h2>
      <div v-if="completedRequests.length > 0" class="completed-grid">
        <!-- Successfully completed items with catalog data -->
        <router-link
          v-for="request in completedRequests"
          :key="request.id"
          :to="getContentLink(request)"
          class="completed-card"
          :class="{ failed: request.status === 'FAILED' }"
        >
          <div class="completed-card-content">
            <!-- Show catalog data if available -->
            <template v-if="request.status === 'COMPLETED' && catalogData[getCatalogKey(request)]">
              <img
                v-if="getCatalogImageUrl(request)"
                :src="getCatalogImageUrl(request)"
                alt="Cover"
                class="completed-image"
              />
              <div v-else class="completed-image-placeholder"></div>
              <div class="completed-info">
                <span class="completed-name">{{ getCatalogName(request) }}</span>
                <span v-if="getCatalogArtist(request)" class="completed-artist">{{ getCatalogArtist(request) }}</span>
                <div class="completed-meta">
                  <span class="status-badge completed">Completed</span>
                  <span v-if="request.completed_at" class="completed-date">
                    {{ formatDate(request.completed_at) }}
                  </span>
                </div>
              </div>
            </template>
            <!-- Fallback for failed or loading items -->
            <template v-else>
              <div class="completed-image-placeholder"></div>
              <div class="completed-info">
                <span class="completed-name">{{ request.content_name }}</span>
                <span v-if="request.artist_name" class="completed-artist">{{ request.artist_name }}</span>
                <div class="completed-meta">
                  <span class="status-badge" :class="getStatusClass(request.status)">
                    {{ formatStatus(request.status) }}
                  </span>
                  <span v-if="request.completed_at" class="completed-date">
                    {{ formatDate(request.completed_at) }}
                  </span>
                </div>
                <span v-if="request.error_message" class="error-message">
                  {{ request.error_message }}
                </span>
              </div>
            </template>
          </div>
        </router-link>
      </div>
      <p v-else class="empty-message">No completed requests</p>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, watch } from "vue";
import { useRouter } from "vue-router";
import { useUserStore } from "@/store/user";
import { formatImageUrl } from "@/utils.js";

const router = useRouter();
const userStore = useUserStore();

const limits = ref(null);
const requests = ref([]);
const catalogData = ref({});
const isLoading = ref(true);

// Redirect if permission is revoked while on this page
watch(
  () => userStore.canRequestContent,
  (canRequest) => {
    if (!canRequest) {
      router.push("/");
    }
  },
);

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
      const data = await response.json();
      // Server returns { requests: [...], stats: {...} }
      requests.value = data.requests || [];
      // Also use the stats if limits weren't fetched separately
      if (data.stats && !limits.value) {
        limits.value = data.stats;
      }
    }
  } catch (error) {
    console.error("Error fetching requests:", error);
  }
};

// Fetch catalog data for a single completed request
const fetchCatalogItem = async (request) => {
  if (request.status !== "COMPLETED" || !request.content_id) return;

  const key = getCatalogKey(request);
  if (catalogData.value[key]) return; // Already fetched

  const typeMap = {
    ALBUM: "album",
    ARTIST: "artist",
    TRACK: "track",
  };
  const contentType = typeMap[request.content_type] || "album";
  const endpoint =
    contentType === "artist"
      ? `/v1/content/artist/${request.content_id}`
      : `/v1/content/${contentType}/${request.content_id}/resolved`;

  try {
    const response = await fetch(endpoint);
    if (response.ok) {
      const data = await response.json();
      catalogData.value = {
        ...catalogData.value,
        [key]: { ...data, type: request.content_type },
      };
    }
  } catch (error) {
    console.error(`Error fetching ${contentType}:`, error);
  }
};

// Fetch catalog data for all completed requests
const fetchCatalogData = async () => {
  const completed = requests.value.filter(
    (r) => r.status === "COMPLETED" && r.content_id
  );
  await Promise.all(completed.map(fetchCatalogItem));
};

// Helper to get catalog key for a request
const getCatalogKey = (request) => {
  return `${request.content_type}:${request.content_id}`;
};

// Helper to get catalog image URL
const getCatalogImageUrl = (request) => {
  const data = catalogData.value[getCatalogKey(request)];
  if (!data?.display_image?.id) return null;
  return formatImageUrl(data.display_image.id);
};

// Helper to get catalog name
const getCatalogName = (request) => {
  const data = catalogData.value[getCatalogKey(request)];
  if (!data) return request.content_name;

  // ResolvedAlbum has album.name, ResolvedArtist has artist.name
  if (data.album) return data.album.name;
  if (data.artist) return data.artist.name;
  return request.content_name;
};

// Helper to get catalog artist name
const getCatalogArtist = (request) => {
  const data = catalogData.value[getCatalogKey(request)];
  if (!data) return request.artist_name;

  // ResolvedAlbum has artists array
  if (data.artists && data.artists.length > 0) {
    return data.artists.map((a) => a.name).join(", ");
  }
  // ResolvedArtist doesn't have artist_name (it IS the artist)
  if (data.artist) {
    return "";
  }
  return request.artist_name || "";
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

const getContentLink = (request) => {
  if (request.status === "FAILED") {
    return "#"; // Failed items don't link anywhere
  }
  // Map content_type to route
  const typeMap = {
    ALBUM: "album",
    ARTIST: "artist",
    TRACK: "track",
  };
  const routeType = typeMap[request.content_type] || "album";
  return `/${routeType}/${request.content_id}`;
};

onMounted(async () => {
  isLoading.value = true;
  await Promise.all([fetchLimits(), fetchRequests()]);
  // Fetch catalog data for completed items (don't block initial render)
  fetchCatalogData();
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

/* Completed Grid */
.completed-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: var(--spacing-3);
}

.completed-card {
  display: block;
  background-color: var(--bg-elevated);
  border-radius: var(--radius-md);
  padding: var(--spacing-3);
  text-decoration: none;
  transition:
    background-color var(--transition-fast),
    transform var(--transition-fast);
}

.completed-card:hover {
  background-color: var(--bg-highlight);
  transform: translateY(-2px);
}

.completed-card.failed {
  opacity: 0.7;
  cursor: default;
}

.completed-card.failed:hover {
  transform: none;
}

.completed-card-content {
  display: flex;
  flex-direction: row;
  align-items: center;
  gap: var(--spacing-3);
}

.completed-image {
  width: 64px;
  height: 64px;
  border-radius: var(--radius-sm);
  object-fit: cover;
  flex-shrink: 0;
}

.completed-image-placeholder {
  width: 64px;
  height: 64px;
  border-radius: var(--radius-sm);
  background-color: var(--bg-subdued);
  flex-shrink: 0;
}

.completed-info {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.completed-name {
  font-weight: var(--font-medium);
  color: var(--text-base);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.completed-artist {
  font-size: var(--text-sm);
  color: var(--text-subdued);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.completed-meta {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  margin-top: var(--spacing-1);
}

.completed-date {
  font-size: var(--text-xs);
  color: var(--text-subdued);
}
</style>
