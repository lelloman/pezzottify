<template>
  <header>
    <div class="topBarContent">
      <router-link to="/" class="logoLink scaleClickFeedback" title="Home">
        <MusicNoteIcon class="logoIcon" />
      </router-link>
      <div class="searchInputContainer">
        <div class="searchBar">
          <input
            class="searchInput"
            type="text"
            placeholder="Search..."
            @input="onInput"
            inputmode="search"
            v-model="localQuery"
          />
          <button
            v-if="localQuery"
            id="clearQueryButton"
            type="submit"
            name="clearQueryButton"
            @click="clearQuery()"
          >
            <CrossIcon class="scaleClickFeedback crossIcon" />
          </button>
        </div>
      </div>
      <div class="userActions">
        <div class="connectionStatus" :title="connectionTitle">
          <span class="statusDot" :class="connectionStatusClass"></span>
        </div>
        <button
          v-if="showIngestionBadge"
          class="ingestionBadge scaleClickFeedback"
          :class="ingestionBadgeClass"
          :title="ingestionBadgeTitle"
          @click="openIngestionMonitor"
        >
          <UploadIcon class="uploadIcon" />
          <span v-if="ingestionStore.activeCount > 0" class="badgeCount">
            {{ ingestionStore.activeCount }}
          </span>
        </button>
        <router-link
          v-if="userStore.hasAnyAdminPermission"
          to="/admin"
          class="adminLink scaleClickFeedback"
          title="Admin Panel"
        >
          <AdminIcon class="adminIcon" />
        </router-link>
        <router-link
          v-if="userStore.canRequestContent"
          to="/requests"
          class="requestsLink scaleClickFeedback"
          title="My Requests"
        >
          <DownloadIcon class="requestsIcon" />
        </router-link>
        <router-link
          to="/settings"
          class="settingsLink scaleClickFeedback"
          title="Settings"
        >
          <SettingsIcon class="settingsIcon" />
        </router-link>
        <router-link
          to="/devices"
          class="devicesLink scaleClickFeedback"
          title="Devices"
        >
          <DevicesIcon class="devicesIcon" />
        </router-link>
        <router-link
          to="/logout"
          class="logoutLink scaleClickFeedback"
          title="Logout"
        >
          <LogoutIcon class="logoutIcon" />
        </router-link>
      </div>
    </div>
  </header>
</template>

<script setup>
import { ref, watch, computed } from "vue";
import { debounce } from "lodash-es"; // Lightweight debounce
import { useRouter, useRoute } from "vue-router";
import CrossIcon from "./icons/CrossIcon.vue";
import SettingsIcon from "./icons/SettingsIcon.vue";
import DevicesIcon from "./icons/DevicesIcon.vue";
import LogoutIcon from "./icons/LogoutIcon.vue";
import AdminIcon from "./icons/AdminIcon.vue";
import DownloadIcon from "./icons/DownloadIcon.vue";
import UploadIcon from "./icons/UploadIcon.vue";
import MusicNoteIcon from "./icons/MusicNoteIcon.vue";
import { wsConnectionStatus, wsServerVersion } from "../services/websocket";
import { useUserStore } from "../store/user";
import { useIngestionStore } from "../store/ingestion";

const userStore = useUserStore();
const ingestionStore = useIngestionStore();

// App version injected by Vite at build time
const appVersion = __APP_VERSION__; // eslint-disable-line no-undef

const emit = defineEmits(["search"]);
const inputValue = ref("");
const router = useRouter();
const route = useRoute();

const props = defineProps({
  initialQuery: {
    type: String,
    default: "",
  },
});

const localQuery = ref(props.initialQuery);
watch(
  () => props.initialQuery,
  (newQuery) => {
    localQuery.value = newQuery;
  },
);

const debounceEmit = debounce((value) => {
  const trimmed = value.trim();
  if (trimmed.length > 0) {
    console.log(
      "TopBar changing search query, current path query: " + route.query,
    );
    router.push({
      path: `/search/${encodeURIComponent(value.trim())}`,
      query: route.query,
    });
  } else {
    router.push({ path: "/" });
  }
  emit("search", value);
}, 300); // 300ms debounce

function onInput(event) {
  inputValue.value = event.target.value;
  debounceEmit(inputValue.value);
}

function clearQuery() {
  router.push("/");
}

// WebSocket connection status indicator
const connectionStatusClass = computed(() => {
  switch (wsConnectionStatus.value) {
    case "connected":
      return "status-connected";
    case "connecting":
      return "status-connecting";
    default:
      return "status-disconnected";
  }
});

const connectionTitle = computed(() => {
  switch (wsConnectionStatus.value) {
    case "connected": {
      const serverVer = wsServerVersion.value || "unknown";
      return `Connected\nWeb: v${appVersion}\nServer: v${serverVer}`;
    }
    case "connecting":
      return `Connecting...\nWeb: v${appVersion}`;
    default:
      return `Disconnected\nWeb: v${appVersion}`;
  }
});

// Ingestion monitor badge
const showIngestionBadge = computed(() => {
  return ingestionStore.badgeState !== "hidden";
});

const ingestionBadgeClass = computed(() => {
  switch (ingestionStore.badgeState) {
    case "active":
      return "badge-active";
    case "review":
      return "badge-review";
    case "complete":
      return "badge-complete";
    default:
      return "";
  }
});

const ingestionBadgeTitle = computed(() => {
  const active = ingestionStore.activeCount;
  const review = ingestionStore.reviewCount;
  const complete = ingestionStore.completedCount;

  if (review > 0) {
    return `${review} upload(s) need review`;
  }
  if (active > 0) {
    return `${active} upload(s) in progress`;
  }
  if (complete > 0) {
    return `${complete} upload(s) complete`;
  }
  return "Ingestion Monitor";
});

function openIngestionMonitor() {
  ingestionStore.openModal();
}
</script>

<style scoped>
.searchInputContainer {
  width: 100%;
  max-width: 31.25rem;
  margin: 1rem auto;
}

.searchBar {
  width: 100%;
  display: flex;
  flex-direction: row;
  align-items: center;
}

.searchInput {
  width: 100%;
  height: 2.8rem;
  background: var(--bg-highlight);
  color: var(--text-base);
  outline: none;
  border: 1px solid var(--border-default);
  border-radius: 1.625rem;
  padding: 0 3.5rem 0 1.5rem;
  font-size: 1rem;
  transition:
    border-color var(--transition-fast),
    background-color var(--transition-fast);
}

.searchInput::placeholder {
  color: var(--text-subdued);
}

.searchInput:focus {
  border-color: var(--spotify-green);
  background: var(--bg-elevated);
}

#clearQueryButton {
  width: 3.5rem;
  height: 2.8rem;
  margin-left: -3.5rem;
  background: none;
  border: none;
  outline: none;
}

#clearQueryButton:hover {
  cursor: pointer;
}

.crossIcon {
  width: 24px;
  height: 24px;
  stroke: #666;
}

.topBarContent {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  height: 100%;
  padding: 0 var(--spacing-4);
  gap: var(--spacing-3);
}

.logoLink {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 40px;
  height: 40px;
  border-radius: var(--radius-full);
  color: var(--spotify-green);
  flex-shrink: 0;
  transition:
    color var(--transition-fast),
    background-color var(--transition-fast);
}

.logoLink:hover {
  background-color: var(--bg-elevated);
}

.logoIcon {
  width: 28px;
  height: 28px;
}

.userActions {
  display: flex;
  align-items: center;
  gap: var(--spacing-3);
  flex-shrink: 0;
}

.adminLink,
.requestsLink,
.settingsLink,
.devicesLink,
.logoutLink {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 40px;
  height: 40px;
  border-radius: var(--radius-full);
  color: var(--text-subdued);
  transition:
    color var(--transition-fast),
    background-color var(--transition-fast);
}

.adminLink:hover,
.requestsLink:hover,
.settingsLink:hover,
.devicesLink:hover,
.logoutLink:hover {
  color: var(--text-base);
  background-color: var(--bg-elevated);
}

.adminIcon,
.requestsIcon,
.settingsIcon,
.devicesIcon,
.logoutIcon {
  width: 20px;
  height: 20px;
}

.connectionStatus {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0 var(--spacing-2);
}

.statusDot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  transition: background-color var(--transition-fast);
}

.status-connected {
  background-color: #22c55e; /* green */
  box-shadow: 0 0 6px rgba(34, 197, 94, 0.5);
}

.status-connecting {
  background-color: #f97316; /* orange */
  box-shadow: 0 0 6px rgba(249, 115, 22, 0.5);
  animation: pulse 1.5s ease-in-out infinite;
}

.status-disconnected {
  background-color: #ef4444; /* red */
  box-shadow: 0 0 6px rgba(239, 68, 68, 0.5);
}

@keyframes pulse {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.5;
  }
}

/* Ingestion badge */
.ingestionBadge {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 40px;
  height: 40px;
  border-radius: var(--radius-full);
  border: none;
  background: transparent;
  color: var(--text-subdued);
  cursor: pointer;
  transition:
    color var(--transition-fast),
    background-color var(--transition-fast);
}

.ingestionBadge:hover {
  color: var(--text-base);
  background-color: var(--bg-elevated);
}

.ingestionBadge.badge-active {
  color: #4a90d9;
  animation: pulse 1.5s ease-in-out infinite;
}

.ingestionBadge.badge-review {
  color: #f5a623;
}

.ingestionBadge.badge-complete {
  color: #7ed321;
}

.uploadIcon {
  width: 20px;
  height: 20px;
}

.badgeCount {
  position: absolute;
  top: 4px;
  right: 4px;
  min-width: 16px;
  height: 16px;
  padding: 0 4px;
  background: #4a90d9;
  color: white;
  border-radius: 8px;
  font-size: 10px;
  font-weight: 600;
  display: flex;
  align-items: center;
  justify-content: center;
}

.badge-review .badgeCount {
  background: #f5a623;
}

.badge-complete .badgeCount {
  background: #7ed321;
}
</style>
