<template>
  <div class="requests-container">
    <header class="page-header">
      <h1 class="page-title">Download requests</h1>
      <p>Follow your requests as they become available in the catalog.</p>
    </header>
    <div v-if="limits" class="limits-section">
      <span
        >Today’s requests
        <strong
          :class="{ warning: limits.requests_today >= limits.max_per_day }"
          >{{ limits.requests_today }} / {{ limits.max_per_day }}</strong
        ></span
      >
      <span class="availability" :class="{ warning: !limits.can_request }"
        ><span class="status-dot" aria-hidden="true"></span
        >{{
          limits.can_request ? "Accepting requests" : "Request limit reached"
        }}</span
      >
    </div>
    <p v-if="isLoading" class="empty-message" role="status">
      Loading requests…
    </p>
    <p v-if="loadError" class="load-error" role="alert">{{ loadError }}</p>
    <template v-if="!isLoading">
      <section
        v-for="group in requestGroups"
        :key="group.title"
        class="section"
        :aria-label="group.title"
      >
        <h2 class="section-title">
          {{ group.title }} <span>{{ group.items.length }}</span>
        </h2>
        <div v-if="group.items.length" class="requests-list">
          <component
            :is="request.status === 'COMPLETED' ? 'router-link' : 'div'"
            v-for="request in group.items"
            :key="request.id"
            :to="
              request.status === 'COMPLETED'
                ? getContentLink(request)
                : undefined
            "
            class="request-row"
            :class="{ 'request-link': request.status === 'COMPLETED' }"
          >
            <img
              v-if="getCatalogImageUrl(request)"
              :src="getCatalogImageUrl(request)"
              alt=""
              class="request-image"
            />
            <div v-else class="request-image placeholder" aria-hidden="true">
              <svg viewBox="0 0 24 24">
                <path d="M9 18V5l11-2v13M9 8l11-2" />
                <ellipse cx="6" cy="18" rx="3" ry="2" />
                <ellipse cx="17" cy="16" rx="3" ry="2" />
              </svg>
            </div>
            <div class="request-info">
              <span class="request-name">{{ getCatalogName(request) }}</span>
              <span class="request-artist"
                >{{ contentTypeLabel(request.content_type)
                }}<template v-if="getCatalogArtist(request)">
                  · {{ getCatalogArtist(request) }}</template
                ></span
              >
              <span v-if="request.error_message" class="error-message">{{
                request.error_message
              }}</span>
            </div>
            <div class="request-status">
              <span class="status-label" :class="getStatusClass(request.status)"
                ><span class="status-dot" aria-hidden="true"></span
                >{{ formatStatus(request.status) }}</span
              >
              <span v-if="request.queue_position" class="secondary-text"
                >#{{ request.queue_position }} in queue</span
              >
              <span v-if="request.completed_at" class="secondary-text">{{
                formatDate(request.completed_at)
              }}</span>
              <div v-if="request.progress" class="progress-details">
                <div
                  class="progress-bar"
                  role="progressbar"
                  :aria-label="`Download progress for ${getCatalogName(request)}`"
                  :aria-valuenow="getProgressPercent(request.progress)"
                  :aria-valuemin="0"
                  :aria-valuemax="100"
                >
                  <div
                    class="progress-fill"
                    :style="{
                      width: getProgressPercent(request.progress) + '%',
                    }"
                  ></div>
                </div>
                <span class="secondary-text"
                  >{{ request.progress.completed || 0 }} /
                  {{ request.progress.total_children }} completed<template
                    v-if="request.progress.failed"
                  >
                    · {{ request.progress.failed }} failed</template
                  ></span
                >
              </div>
            </div>
          </component>
        </div>
        <p v-else class="empty-message">{{ group.empty }}</p>
      </section>
    </template>
  </div>
</template>

<script setup>
import {
  ref,
  computed,
  onMounted,
  onActivated,
  onDeactivated,
  onUnmounted,
  watch,
} from "vue";
import { useRouter } from "vue-router";
import { useUserStore } from "@/store/user";
import { useRemoteStore } from "@/store/remote";
import { formatImageUrl } from "@/utils.js";

const router = useRouter();
const userStore = useUserStore();
const remoteStore = useRemoteStore();

const limits = ref(null);
const requests = ref([]);
const catalogData = ref({});
const isLoading = ref(true);
const loadError = ref("");

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
    (r) => !["COMPLETED", "FAILED"].includes(r.status),
  );
});

const completedRequests = computed(() => {
  return requests.value.filter((r) =>
    ["COMPLETED", "FAILED"].includes(r.status),
  );
});

const requestGroups = computed(() => [
  {
    title: "In progress",
    items: pendingRequests.value,
    empty:
      "No active requests. Request an album or track from its catalog page.",
  },
  {
    title: "History",
    items: completedRequests.value,
    empty: "Finished requests will appear here.",
  },
]);
const contentTypeLabel = (type) =>
  ({ ALBUM: "Album", ARTIST: "Artist", TRACK: "Track" })[type] || "Content";

const fetchLimits = async () => {
  try {
    const data = await remoteStore.fetchDownloadLimits();
    if (data) {
      limits.value = data;
    }
  } catch (error) {
    console.error("Error fetching limits:", error);
  }
};

const fetchRequests = async () => {
  try {
    const data = await remoteStore.fetchMyDownloadRequests();
    if (data) {
      loadError.value = "";
      // Server returns { requests: [...], stats: {...} }
      requests.value = data.requests || [];
      // Also use the stats if limits weren't fetched separately
      if (data.stats && !limits.value) {
        limits.value = data.stats;
      }
    } else {
      loadError.value = "Could not load requests. Please try again shortly.";
    }
  } catch (error) {
    loadError.value = "Could not load requests. Please try again shortly.";
    console.error("Error fetching requests:", error);
  }
};

// Fetch catalog data for a single completed request
const fetchCatalogItem = async (request) => {
  if (request.status !== "COMPLETED" || !request.content_id) return;

  const key = getCatalogKey(request);
  if (catalogData.value[key]) return; // Already fetched

  try {
    let data = null;
    switch (request.content_type) {
      case "ALBUM":
        data = await remoteStore.fetchResolvedAlbum(request.content_id);
        break;
      case "ARTIST":
        data = await remoteStore.fetchArtist(request.content_id);
        break;
      case "TRACK":
        data = await remoteStore.fetchResolvedTrack(request.content_id);
        break;
    }
    if (data) {
      catalogData.value = {
        ...catalogData.value,
        [key]: { ...data, type: request.content_type },
      };
    }
  } catch (error) {
    console.error(`Error fetching catalog item:`, error);
  }
};

// Fetch catalog data for all completed requests
const fetchCatalogData = async () => {
  const completed = requests.value.filter(
    (r) => r.status === "COMPLETED" && r.content_id,
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
  if (data.track) return data.track.name || data.track.title;
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
    return data.artists
      .map((a) => a.artist?.name || a.name)
      .filter(Boolean)
      .join(", ");
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
  if (!progress || !(progress.total_children > 0)) return 0;
  return Math.max(
    0,
    Math.min(
      100,
      Math.round(
        (((progress.completed || 0) + (progress.failed || 0)) /
          progress.total_children) *
          100,
      ),
    ),
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

// Auto-refresh while there are pending requests
let autoRefreshInterval = null;

const startAutoRefresh = () => {
  if (autoRefreshInterval) return;
  autoRefreshInterval = setInterval(async () => {
    if (pendingRequests.value.length > 0) {
      await Promise.all([fetchLimits(), fetchRequests()]);
      fetchCatalogData();
    } else {
      stopAutoRefresh();
    }
  }, 10000);
};

const stopAutoRefresh = () => {
  if (autoRefreshInterval) {
    clearInterval(autoRefreshInterval);
    autoRefreshInterval = null;
  }
};

// Re-fetch when sync events update download state
watch(
  () => userStore.downloadRequests,
  () => {
    fetchRequests().then(() => fetchCatalogData());
  },
);

onMounted(async () => {
  isLoading.value = true;
  await Promise.all([fetchLimits(), fetchRequests()]);
  // Fetch catalog data for completed items (don't block initial render)
  fetchCatalogData();
  isLoading.value = false;
  if (pendingRequests.value.length > 0) {
    startAutoRefresh();
  }
});

onActivated(() => {
  if (pendingRequests.value.length > 0) {
    startAutoRefresh();
  }
});

onUnmounted(stopAutoRefresh);

onDeactivated(() => {
  stopAutoRefresh();
});
</script>

<style scoped>
.requests-container {
  display: flex;
  flex-direction: column;
  width: 100%;
  gap: 28px;
  color: var(--text-base);
}
.page-header {
  padding-top: 16px;
}
.page-title {
  margin: 0 0 12px;
  font-size: 32px;
  font-weight: 700;
  letter-spacing: -0.03em;
  line-height: 1.2;
}
.page-header p {
  margin: 0;
  font-size: 14px;
  color: var(--text-subdued);
  line-height: 1.5;
}
.limits-section {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 12px 28px;
  padding-bottom: 24px;
  border-bottom: 1px solid var(--surface-border);
  font-size: 14px;
  color: var(--text-subdued);
}
.limits-section strong {
  color: var(--text-base);
  margin-left: 12px;
  font-weight: 500;
  font-variant-numeric: tabular-nums;
}
.availability,
.status-label {
  display: inline-flex;
  align-items: center;
  gap: 8px;
}
.availability .status-dot {
  color: var(--spotify-green);
}
.status-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: currentColor;
  flex-shrink: 0;
}
.limits-section .warning,
.warning .status-dot {
  color: #f0bc65;
}
.section-title {
  display: flex;
  align-items: baseline;
  gap: 12px;
  margin: 0 0 16px;
  font-size: 24px;
  font-weight: 700;
  letter-spacing: -0.02em;
}
.section-title span {
  color: var(--text-subdued);
  font-size: 14px;
  font-weight: 400;
  font-variant-numeric: tabular-nums;
  letter-spacing: 0;
}
.requests-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.request-row {
  display: grid;
  grid-template-columns: 48px minmax(0, 1fr) minmax(140px, 220px);
  gap: 16px;
  align-items: center;
  padding: 12px;
  border-radius: 4px;
  color: inherit;
  text-decoration: none;
}
.request-link:hover {
  background: var(--surface-hover);
}
.request-link:focus-visible {
  outline: 2px solid white;
  outline-offset: -2px;
}
.request-link:hover .request-name {
  text-decoration: underline;
}
.request-image {
  width: 48px;
  height: 48px;
  border-radius: 4px;
  object-fit: cover;
}
.placeholder {
  display: grid;
  place-items: center;
  background: #242424;
  color: var(--text-subdued);
}
.placeholder svg {
  width: 24px;
  height: 24px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.5;
}
.request-info,
.request-status {
  display: flex;
  flex-direction: column;
  min-width: 0;
  gap: 5px;
}
.request-name {
  font-size: 16px;
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.request-artist {
  font-size: 14px;
  color: var(--text-subdued);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.status-label {
  font-size: 13px;
  color: var(--text-subdued);
}
.status-label.in-progress {
  color: var(--spotify-green);
}
.status-label.retry-waiting {
  color: #f0bc65;
}
.status-label.failed,
.error-message,
.load-error {
  color: #f3727f;
}
.secondary-text,
.error-message {
  font-size: 12px;
  line-height: 1.5;
}
.secondary-text {
  color: var(--text-subdued);
  font-variant-numeric: tabular-nums;
}
.error-message {
  overflow-wrap: anywhere;
}
.progress-details {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-top: 4px;
}
.progress-bar {
  height: 4px;
  background: #535353;
  border-radius: 999px;
  overflow: hidden;
}
.progress-fill {
  height: 100%;
  background: var(--spotify-green);
  border-radius: inherit;
}
.empty-message {
  margin: 0;
  padding: 24px;
  background: #181818;
  border-radius: 8px;
  font-size: 14px;
  line-height: 1.6;
  color: var(--text-subdued);
}
@container (max-width: 550px) {
  .request-row {
    grid-template-columns: 48px minmax(0, 1fr);
    gap: 12px;
    padding: 12px 0;
  }
  .request-status {
    grid-column: 2;
  }
}
</style>
