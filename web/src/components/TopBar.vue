<template>
  <header>
    <div class="topBarContent">
      <router-link
        to="/"
        class="logoLink scaleClickFeedback"
        title="Pezzottify Home"
      >
        <MusicNoteIcon class="logoIcon" />
        <span class="logoWordmark">ezzottify</span>
      </router-link>
      <div ref="searchContainerRef" class="searchInputContainer">
        <div class="searchBar">
          <input
            class="searchInput"
            type="text"
            placeholder="Search..."
            @focus="handleSearchFocus"
            @input="onInput"
            @keydown.enter.prevent="commitSearch"
            @keydown.esc.prevent="closeSearchPopover"
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

        <div
          v-if="isSearchPopoverOpen"
          class="searchPopover"
          @mousedown.prevent
        >
          <template v-if="hasSuggestionQuery">
            <div class="popoverHeader">
              <span>Search suggestions</span>
              <button
                type="button"
                class="searchAllButton"
                @click="commitSearch"
              >
                Search all
              </button>
            </div>

            <div
              v-if="isSuggestionLoading && suggestionSections.length === 0"
              class="popoverState"
            >
              Searching
            </div>
            <div v-else-if="suggestionError" class="popoverState">
              Search is unavailable
            </div>
            <div
              v-else-if="
                !isSuggestionLoading && suggestionSections.length === 0
              "
              class="popoverState"
            >
              No matches
            </div>
            <div v-else class="suggestionSections">
              <section
                v-for="section in suggestionSections"
                :key="section.type"
                class="suggestionSection"
              >
                <h3 class="suggestionSectionTitle">{{ section.label }}</h3>
                <button
                  v-for="result in section.results"
                  :key="result.type + '-' + result.id"
                  type="button"
                  class="suggestionRow"
                  @click="selectSuggestion(result)"
                >
                  <MultiSourceImage
                    :urls="suggestionImageUrls(result)"
                    :lazy="false"
                    :class="{
                      suggestionImage: true,
                      roundSuggestionImage: result.type === 'Artist',
                    }"
                  />
                  <span class="suggestionText">
                    <span class="suggestionTitle">{{ result.name }}</span>
                    <span class="suggestionSubtitle">
                      {{ suggestionSubtitle(result) }}
                    </span>
                  </span>
                </button>
              </section>
            </div>
          </template>

          <template v-else>
            <section v-if="recentSearches.length" class="suggestionSection">
              <h3 class="suggestionSectionTitle">Recent searches</h3>
              <button
                v-for="query in recentSearches"
                :key="query"
                type="button"
                class="recentSearchButton"
                @click="runRecentSearch(query)"
              >
                {{ query }}
              </button>
            </section>

            <section class="suggestionSection">
              <h3 class="suggestionSectionTitle">Quick links</h3>
              <div class="quickLinkGrid">
                <button
                  v-for="link in quickLinks"
                  :key="link.path"
                  type="button"
                  class="quickLinkButton"
                  @click="openQuickLink(link.path)"
                >
                  {{ link.label }}
                </button>
              </div>
            </section>
          </template>
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
import { ref, watch, computed, onMounted, onBeforeUnmount } from "vue";
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
import MultiSourceImage from "./common/MultiSourceImage.vue";
import { formatImageUrl } from "../utils";
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
const searchContainerRef = ref(null);
const isSearchPopoverOpen = ref(false);
const isSuggestionLoading = ref(false);
const suggestionError = ref(false);
const searchSuggestions = ref([]);

const RECENT_SEARCHES_KEY = "pezzottify_recent_searches";
const MAX_RECENT_SEARCHES = 5;
const SUGGESTION_LIMIT = 8;
let suggestionAbortController = null;
const suggestionImageUrlCache = new Map();

function loadRecentSearches() {
  try {
    const saved = JSON.parse(localStorage.getItem(RECENT_SEARCHES_KEY) || "[]");
    return Array.isArray(saved)
      ? saved.filter(Boolean).slice(0, MAX_RECENT_SEARCHES)
      : [];
  } catch {
    return [];
  }
}

const recentSearches = ref(loadRecentSearches());

const quickLinks = computed(() => {
  const links = [
    { label: "Genres", path: "/genres" },
    { label: "Shows", path: "/shows" },
    { label: "Devices", path: "/devices" },
  ];

  if (userStore.canRequestContent) {
    links.splice(2, 0, { label: "Requests", path: "/requests" });
  }

  return links;
});

const props = defineProps({
  initialQuery: {
    type: String,
    default: "",
  },
});

const localQuery = ref(props.initialQuery);
const hasSuggestionQuery = computed(() => localQuery.value.trim().length > 0);
const suggestionSections = computed(() => {
  const sections = [
    { type: "Track", label: "Tracks", results: [] },
    { type: "Album", label: "Albums", results: [] },
    { type: "Artist", label: "Artists", results: [] },
  ];

  for (const result of searchSuggestions.value) {
    const section = sections.find((item) => item.type === result.type);
    if (section && section.results.length < 3) {
      section.results.push(result);
    }
  }

  return sections.filter((section) => section.results.length > 0);
});

watch(
  () => props.initialQuery,
  (newQuery) => {
    localQuery.value = newQuery;
  },
);

function saveRecentSearch(query) {
  const trimmed = query.trim();
  if (!trimmed) return;

  recentSearches.value = [
    trimmed,
    ...recentSearches.value.filter(
      (item) => item.toLowerCase() !== trimmed.toLowerCase(),
    ),
  ].slice(0, MAX_RECENT_SEARCHES);
  localStorage.setItem(
    RECENT_SEARCHES_KEY,
    JSON.stringify(recentSearches.value),
  );
}

const fetchSuggestions = debounce(async (query) => {
  const trimmed = query.trim();
  if (!trimmed) {
    searchSuggestions.value = [];
    isSuggestionLoading.value = false;
    suggestionError.value = false;
    return;
  }

  suggestionAbortController?.abort();
  suggestionAbortController = new AbortController();
  isSuggestionLoading.value = true;
  suggestionError.value = false;

  try {
    const response = await fetch("/v1/content/search", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        query: trimmed,
        resolve: true,
        limit: SUGGESTION_LIMIT,
        exclude_unavailable: true,
      }),
      signal: suggestionAbortController.signal,
    });

    if (!response.ok) {
      throw new Error(`Search suggestions failed: ${response.status}`);
    }

    const payload = await response.json();
    searchSuggestions.value = Array.isArray(payload) ? payload : [];
  } catch (error) {
    if (error.name !== "AbortError") {
      console.error("Search suggestion error:", error);
      suggestionError.value = true;
      searchSuggestions.value = [];
    }
  } finally {
    isSuggestionLoading.value = false;
  }
}, 500);

function queueSuggestionFetch(query) {
  const trimmed = query.trim();
  if (!trimmed) {
    fetchSuggestions.cancel();
    suggestionAbortController?.abort();
    searchSuggestions.value = [];
    isSuggestionLoading.value = false;
    suggestionError.value = false;
    return;
  }

  isSuggestionLoading.value = true;
  suggestionError.value = false;
  fetchSuggestions(trimmed);
}

function onInput(event) {
  inputValue.value = event.target.value;
  isSearchPopoverOpen.value = true;
  queueSuggestionFetch(inputValue.value);
}

function handleSearchFocus() {
  isSearchPopoverOpen.value = true;
  queueSuggestionFetch(localQuery.value);
}

function closeSearchPopover() {
  isSearchPopoverOpen.value = false;
}

function commitSearch() {
  const trimmed = localQuery.value.trim();
  if (!trimmed) return;

  saveRecentSearch(trimmed);
  closeSearchPopover();
  router.push({
    path: `/search/${encodeURIComponent(trimmed)}`,
    query: route.query,
  });
  emit("search", trimmed);
}

function clearQuery() {
  localQuery.value = "";
  inputValue.value = "";
  searchSuggestions.value = [];
  suggestionError.value = false;
  router.push("/");
}

function runRecentSearch(query) {
  localQuery.value = query;
  inputValue.value = query;
  commitSearch();
}

function openQuickLink(path) {
  closeSearchPopover();
  router.push(path);
}

function resultPath(result) {
  switch (result.type) {
    case "Album":
      return `/album/${result.id}`;
    case "Artist":
      return `/artist/${result.id}`;
    case "Track":
      return `/track/${result.id}`;
    default:
      return "/";
  }
}

function selectSuggestion(result) {
  saveRecentSearch(result.name);
  closeSearchPopover();
  router.push(resultPath(result));
}

function artistNames(artistsIdsNames) {
  if (!Array.isArray(artistsIdsNames)) return "";
  return artistsIdsNames
    .map((artist) => artist.name || artist[1])
    .filter(Boolean)
    .join(", ");
}

function suggestionSubtitle(result) {
  switch (result.type) {
    case "Album": {
      const artists = artistNames(result.artists_ids_names);
      return [result.year, artists].filter(Boolean).join(" - ");
    }
    case "Artist":
      return "Artist";
    case "Track":
      return artistNames(result.artists_ids_names) || "Track";
    default:
      return result.type;
  }
}

function suggestionImageUrls(result) {
  const imageId = result.type === "Track" ? result.album_id : result.id;
  const cacheKey = `${result.type}-${result.id}-${imageId || ""}`;

  if (!suggestionImageUrlCache.has(cacheKey)) {
    suggestionImageUrlCache.set(
      cacheKey,
      imageId ? [formatImageUrl(imageId)] : [],
    );
  }

  return suggestionImageUrlCache.get(cacheKey);
}

function handleDocumentPointerDown(event) {
  if (!searchContainerRef.value?.contains(event.target)) {
    closeSearchPopover();
  }
}

onMounted(() => {
  document.addEventListener("pointerdown", handleDocumentPointerDown);
});

onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", handleDocumentPointerDown);
  suggestionAbortController?.abort();
  fetchSuggestions.cancel();
});

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
header {
  position: relative;
  z-index: var(--z-sticky);
  height: var(--topbar-height);
  background: #0b0d0e;
  border-bottom: 1px solid var(--surface-border);
}

.searchInputContainer {
  position: relative;
  width: 100%;
  max-width: 38rem;
  margin: 0 auto;
}

.searchBar {
  width: 100%;
  display: flex;
  flex-direction: row;
  align-items: center;
}

.searchInput {
  width: 100%;
  height: 2.7rem;
  background: #1a1f22;
  color: var(--text-base);
  outline: none;
  border: 1px solid var(--surface-border);
  border-radius: 8px;
  padding: 0 3.5rem 0 1rem;
  font-size: 0.94rem;
  font-weight: 650;
  transition:
    border-color var(--transition-fast),
    background-color var(--transition-fast);
}

.searchInput::placeholder {
  color: var(--text-subdued);
}

.searchInput:focus {
  border-color: rgba(29, 185, 84, 0.52);
  background: #151a1d;
  box-shadow: 0 0 0 3px rgba(29, 185, 84, 0.12);
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

.searchPopover {
  position: absolute;
  top: calc(100% + 8px);
  left: 0;
  right: 0;
  z-index: var(--z-dropdown);
  max-height: min(68vh, 520px);
  overflow-y: auto;
  padding: 10px;
  background: #111416;
  border: 1px solid var(--surface-border-strong);
  border-radius: 8px;
  box-shadow: 0 18px 44px rgba(0, 0, 0, 0.62);
}

.popoverHeader {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 2px 2px 10px;
  color: var(--text-subdued);
  font-size: var(--text-xs);
  font-weight: var(--font-semibold);
  text-transform: uppercase;
}

.searchAllButton {
  min-height: 28px;
  padding: 0 10px;
  border: 1px solid var(--surface-border-strong);
  border-radius: 6px;
  background: #1a1f22;
  color: var(--text-base);
  cursor: pointer;
  font-size: var(--text-xs);
  font-weight: var(--font-semibold);
}

.searchAllButton:hover {
  background: #252b2f;
}

.popoverState {
  padding: 22px 8px;
  color: var(--text-subdued);
  text-align: center;
  font-size: var(--text-sm);
}

.suggestionSections {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.suggestionSection {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.suggestionSection + .suggestionSection {
  padding-top: 6px;
  border-top: 1px solid var(--surface-border);
}

.suggestionSectionTitle {
  margin: 0;
  padding: 0 2px;
  color: var(--text-subdued);
  font-size: var(--text-xs);
  font-weight: var(--font-semibold);
  text-transform: uppercase;
}

.suggestionRow,
.recentSearchButton,
.quickLinkButton {
  appearance: none;
  border: 1px solid transparent;
  cursor: pointer;
  font: inherit;
  text-align: left;
  transition:
    background-color var(--transition-fast),
    border-color var(--transition-fast);
}

.suggestionRow {
  display: flex;
  align-items: center;
  gap: 10px;
  min-height: 54px;
  width: 100%;
  padding: 7px;
  border-radius: 8px;
  background: #111416;
}

.suggestionRow:hover,
.recentSearchButton:hover,
.quickLinkButton:hover {
  background: #1b2023;
  border-color: rgba(255, 255, 255, 0.08);
}

.suggestionImage {
  width: 40px;
  height: 40px;
  flex: 0 0 auto;
  border-radius: 7px;
  background: var(--bg-highlight);
  object-fit: cover;
}

.roundSuggestionImage {
  border-radius: 50%;
}

.suggestionText {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.suggestionTitle,
.suggestionSubtitle {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.suggestionTitle {
  color: var(--text-base);
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
}

.suggestionSubtitle {
  color: var(--text-subdued);
  font-size: var(--text-xs);
}

.recentSearchButton {
  min-height: 34px;
  padding: 0 10px;
  border-radius: 7px;
  background: #151a1d;
  color: var(--text-base);
  font-weight: var(--font-semibold);
}

.quickLinkGrid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 6px;
}

.quickLinkButton {
  min-height: 38px;
  padding: 0 10px;
  border-radius: 7px;
  background: #1a1f22;
  color: var(--text-base);
  font-weight: var(--font-semibold);
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
  padding: 0 14px;
  gap: 12px;
}

.logoLink {
  display: flex;
  align-items: center;
  justify-content: flex-start;
  gap: 0;
  min-width: 40px;
  height: 40px;
  padding: 0 8px 0 4px;
  border-radius: 8px;
  color: var(--spotify-green);
  flex-shrink: 0;
  transition:
    color var(--transition-fast),
    background-color var(--transition-fast);
}

.logoLink:hover {
  background-color: var(--surface-hover);
}

.logoIcon {
  width: 28px;
  height: 28px;
  flex-shrink: 0;
}

.logoWordmark {
  color: var(--spotify-green);
  font-size: 1.08rem;
  font-weight: var(--font-bold);
  line-height: 1;
  margin-left: -8px;
  transform: translateY(1px);
  white-space: nowrap;
}

.userActions {
  display: flex;
  align-items: center;
  gap: 6px;
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
  border-radius: 8px;
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
  background-color: var(--surface-hover);
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
  border-radius: 8px;
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
  background-color: var(--surface-hover);
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
