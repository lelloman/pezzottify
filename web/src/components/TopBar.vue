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
      <div
        ref="searchContainerRef"
        class="searchInputContainer"
        @focusout="handleSearchFocusOut"
      >
        <div class="searchBar">
          <svg class="searchGlyph" viewBox="0 0 24 24" aria-hidden="true">
            <circle cx="10.5" cy="10.5" r="7.5" />
            <path d="m16 16 5 5" />
          </svg>
          <input
            class="searchInput"
            type="text"
            placeholder="What do you want to play?"
            aria-label="Search music"
            :aria-expanded="isSearchPopoverOpen"
            aria-controls="search-suggestions"
            autocomplete="off"
            @keydown.down.prevent="focusSuggestion"
            @focus="handleSearchFocus"
            @click="!isSearchPopoverOpen && handleSearchFocus()"
            @input="onInput"
            @keydown.enter.prevent="commitSearch"
            @keydown.esc.prevent="closeSearchPopover"
            inputmode="search"
            v-model="localQuery"
          />
          <button
            v-if="localQuery"
            id="clearQueryButton"
            type="button"
            aria-label="Clear search"
            name="clearQueryButton"
            @click="clearQuery()"
          >
            <svg class="crossIcon" viewBox="0 0 24 24" aria-hidden="true">
              <path d="m5 5 14 14M5 19 19 5" />
            </svg>
          </button>
        </div>

        <div
          v-if="isSearchPopoverOpen"
          id="search-suggestions"
          class="searchPopover"
          @keydown="navigateSuggestions"
        >
          <template v-if="hasSuggestionQuery">
            <div class="popoverHeader">
              <span>↑ ↓ Navigate</span><span>Enter to open</span>
            </div>
            <button
              type="button"
              class="suggestionRow searchQueryRow"
              data-search-choice
              @click="commitSearch"
            >
              <svg class="queryIcon" viewBox="0 0 24 24" aria-hidden="true">
                <circle cx="10" cy="10" r="7" />
                <path d="m15 15 6 6" />
              </svg>
              <span class="suggestionTitle"
                >Search for “{{ localQuery.trim() }}”</span
              >
            </button>
            <p
              v-if="isSuggestionLoading && !visibleSuggestions.length"
              class="popoverState"
              role="status"
            >
              Searching…
            </p>
            <p v-else-if="suggestionError" class="popoverState" role="status">
              Search is unavailable
            </p>
            <p
              v-else-if="!isSuggestionLoading && !visibleSuggestions.length"
              class="popoverState"
              role="status"
            >
              No matches
            </p>
            <button
              v-for="result in visibleSuggestions"
              :key="result.type + '-' + result.id"
              type="button"
              class="suggestionRow"
              data-search-choice
              @click="selectSuggestion(result)"
            >
              <MultiSourceImage
                :urls="suggestionImageUrls(result)"
                :lazy="false"
                class="suggestionImage"
                :class="{ roundSuggestionImage: result.type === 'Artist' }"
                alt=""
              />
              <span class="suggestionText"
                ><span class="suggestionTitle">{{ result.name }}</span
                ><span class="suggestionSubtitle">{{
                  suggestionSubtitle(result)
                }}</span></span
              >
            </button>
          </template>
          <template v-else>
            <h3 class="suggestionSectionTitle">Recent searches</h3>
            <div
              v-for="(entry, index) in recentSearches"
              :key="entry.type + ':' + (entry.id || entry.name)"
              class="recentSearchRow"
            >
              <button
                type="button"
                class="suggestionRow"
                data-search-choice
                @click="runRecentSearch(entry)"
              >
                <MultiSourceImage
                  v-if="entry.type !== 'Query'"
                  :urls="suggestionImageUrls(entry)"
                  :lazy="false"
                  class="suggestionImage"
                  :class="{ roundSuggestionImage: entry.type === 'Artist' }"
                  alt=""
                />
                <svg
                  v-else
                  class="queryIcon"
                  viewBox="0 0 24 24"
                  aria-hidden="true"
                >
                  <circle cx="12" cy="12" r="9" />
                  <path d="M12 6v6l4 2" />
                </svg>
                <span class="suggestionText"
                  ><span class="suggestionTitle">{{ entry.name }}</span
                  ><span class="suggestionSubtitle">{{
                    entry.type === "Query"
                      ? "Search"
                      : suggestionSubtitle(entry)
                  }}</span></span
                >
              </button>
              <button
                type="button"
                class="removeRecent"
                :aria-label="`Remove ${entry.name} from recent searches`"
                @click="removeRecentSearch(index)"
              >
                <svg viewBox="0 0 24 24" aria-hidden="true">
                  <path d="m5 5 14 14M5 19 19 5" />
                </svg>
              </button>
            </div>
            <p v-if="!recentSearches.length" class="popoverState">
              Your recent searches will appear here.
            </p>
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
          class="adminLink"
          title="Admin Panel"
        >
          <AdminIcon class="adminIcon" />
        </router-link>
        <router-link
          v-if="userStore.canRequestContent"
          to="/requests"
          class="requestsLink"
          title="My Requests"
        >
          <DownloadIcon class="requestsIcon" />
        </router-link>
        <router-link to="/settings" class="settingsLink" title="Settings">
          <SettingsIcon class="settingsIcon" />
        </router-link>
        <router-link to="/devices" class="devicesLink" title="Devices">
          <DevicesIcon class="devicesIcon" />
        </router-link>
        <router-link to="/logout" class="logoutLink" title="Logout">
          <LogoutIcon class="logoutIcon" />
        </router-link>
      </div>
    </div>
  </header>
</template>

<script setup>
import {
  ref,
  watch,
  computed,
  nextTick,
  onMounted,
  onBeforeUnmount,
} from "vue";
import { storeToRefs } from "pinia";
import { debounce } from "lodash-es"; // Lightweight debounce
import { useRouter, useRoute } from "vue-router";
import SettingsIcon from "./icons/SettingsIcon.vue";
import DevicesIcon from "./icons/DevicesIcon.vue";
import LogoutIcon from "./icons/LogoutIcon.vue";
import AdminIcon from "./icons/AdminIcon.vue";
import DownloadIcon from "./icons/DownloadIcon.vue";
import UploadIcon from "./icons/UploadIcon.vue";
import MusicNoteIcon from "./icons/MusicNoteIcon.vue";
import MultiSourceImage from "./common/MultiSourceImage.vue";
import { formatImageUrl } from "../utils";
import { authenticatedFetch } from "../services/authenticatedFetch.js";
import { wsConnectionStatus, wsServerVersion } from "../services/websocket";
import {
  fetchStreamingSearchSections,
  sectionsToResults,
} from "../services/streamingSearch";
import { useUserStore } from "../store/user";
import { useIngestionStore } from "../store/ingestion";
import { useDebugStore } from "../store/debug";

const userStore = useUserStore();
const ingestionStore = useIngestionStore();
const debugStore = useDebugStore();
const { excludeUnavailable, useOrganicSearch } = storeToRefs(debugStore);

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
const MAX_RECENT_SEARCHES = 10;
const SUGGESTION_FETCH_LIMIT = 30;
let suggestionAbortController = null;
let suggestionVersion = 0;
const suggestionImageUrlCache = new Map();

function loadRecentSearches() {
  try {
    const saved = JSON.parse(localStorage.getItem(RECENT_SEARCHES_KEY) || "[]");
    return Array.isArray(saved)
      ? saved
          .map((entry) =>
            typeof entry === "string" ? { type: "Query", name: entry } : entry,
          )
          .filter(
            (entry) =>
              entry &&
              typeof entry.name === "string" &&
              ["Query", "Album", "Artist", "Track"].includes(entry.type) &&
              (entry.type === "Query" || typeof entry.id === "string"),
          )
          .slice(0, MAX_RECENT_SEARCHES)
      : [];
  } catch {
    return [];
  }
}

const recentSearches = ref(loadRecentSearches());

const props = defineProps({
  initialQuery: {
    type: String,
    default: "",
  },
});

const localQuery = ref(props.initialQuery);
const hasSuggestionQuery = computed(() => localQuery.value.trim().length > 0);
const visibleSuggestions = computed(() =>
  searchSuggestions.value
    .filter((result) => ["Track", "Album", "Artist"].includes(result.type))
    .slice(0, 9),
);

watch(
  () => props.initialQuery,
  (newQuery) => {
    localQuery.value = newQuery;
  },
);

function persistRecentSearches() {
  try {
    localStorage.setItem(
      RECENT_SEARCHES_KEY,
      JSON.stringify(recentSearches.value),
    );
  } catch {
    /* Keep history in memory if storage is unavailable. */
  }
}
function saveRecentSearch(value) {
  const entry =
    typeof value === "string"
      ? { type: "Query", name: value.trim() }
      : {
          type: value.type,
          id: value.id,
          name: value.name,
          album_id: value.album_id,
          artists_ids_names: value.artists_ids_names,
          year: value.year,
        };
  if (!entry.name) return;
  recentSearches.value = [
    entry,
    ...recentSearches.value.filter(
      (item) =>
        !(
          item.type === entry.type &&
          (entry.id
            ? item.id === entry.id
            : item.name.toLowerCase() === entry.name.toLowerCase())
        ),
    ),
  ].slice(0, MAX_RECENT_SEARCHES);
  persistRecentSearches();
}
function removeRecentSearch(index) {
  recentSearches.value.splice(index, 1);
  persistRecentSearches();
  nextTick(() => {
    const target =
      searchContainerRef.value?.querySelectorAll(".removeRecent")[
        Math.min(index, recentSearches.value.length - 1)
      ] || searchContainerRef.value?.querySelector("input");
    target?.focus();
  });
}
function focusSuggestion() {
  isSearchPopoverOpen.value = true;
  nextTick(() =>
    searchContainerRef.value?.querySelector("[data-search-choice]")?.focus(),
  );
}
function navigateSuggestions(event) {
  if (event.key === "Escape") {
    event.preventDefault();
    searchContainerRef.value?.querySelector("input")?.focus();
    closeSearchPopover();
    return;
  }
  if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) return;
  const rows = [
    ...searchContainerRef.value.querySelectorAll("[data-search-choice]"),
  ];
  const index = rows.indexOf(document.activeElement);
  event.preventDefault();
  if (event.key === "ArrowUp" && index <= 0) {
    searchContainerRef.value.querySelector("input").focus();
    return;
  }
  const next =
    event.key === "Home"
      ? 0
      : event.key === "End"
        ? rows.length - 1
        : Math.max(
            0,
            Math.min(
              rows.length - 1,
              index + (event.key === "ArrowDown" ? 1 : -1),
            ),
          );
  rows[next]?.focus();
}

const fetchSuggestions = debounce(async (query, version) => {
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
    if (useOrganicSearch.value) {
      const response = await authenticatedFetch("/v1/content/search", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          query: trimmed,
          resolve: true,
          limit: SUGGESTION_FETCH_LIMIT,
          exclude_unavailable:
            excludeUnavailable.value && !userStore.isProxyModeEnabled,
          search_mode: "expanded",
        }),
        signal: suggestionAbortController.signal,
      });

      if (!response.ok) {
        throw new Error(`Search suggestions failed: ${response.status}`);
      }

      const payload = await response.json();
      if (version !== suggestionVersion) return;
      searchSuggestions.value = Array.isArray(payload) ? payload : [];
    } else {
      const sections = await fetchStreamingSearchSections(
        trimmed,
        {
          excludeUnavailable:
            excludeUnavailable.value && !userStore.isProxyModeEnabled,
        },
        suggestionAbortController.signal,
      );
      if (version !== suggestionVersion) return;
      searchSuggestions.value = sectionsToResults(sections).slice(
        0,
        SUGGESTION_FETCH_LIMIT,
      );
    }
  } catch (error) {
    if (version === suggestionVersion && error.name !== "AbortError") {
      console.error("Search suggestion error:", error);
      suggestionError.value = true;
      searchSuggestions.value = [];
    }
  } finally {
    if (version === suggestionVersion) isSuggestionLoading.value = false;
  }
}, 300);

function queueSuggestionFetch(query) {
  const version = ++suggestionVersion;
  suggestionAbortController?.abort();
  searchSuggestions.value = [];
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
  fetchSuggestions(trimmed, version);
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
  queueSuggestionFetch("");
  localQuery.value = "";
  inputValue.value = "";
  searchSuggestions.value = [];
  suggestionError.value = false;
  router.push("/");
  nextTick(() => searchContainerRef.value?.querySelector("input")?.focus());
}

function runRecentSearch(entry) {
  if (entry.type !== "Query") return selectSuggestion(entry);
  localQuery.value = entry.name;
  inputValue.value = entry.name;
  commitSearch();
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
  saveRecentSearch(result);
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
      return ["Album", result.year, artists].filter(Boolean).join(" · ");
    }
    case "Artist":
      return "Artist";
    case "Track":
      return ["Song", artistNames(result.artists_ids_names)]
        .filter(Boolean)
        .join(" · ");
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

function handleSearchFocusOut(event) {
  if (
    event.relatedTarget &&
    !searchContainerRef.value?.contains(event.relatedTarget)
  )
    closeSearchPopover();
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
  background: var(--bg-base);
  border-bottom: 0;
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
  height: 48px;
  background: var(--surface-raised);
  color: var(--text-base);
  outline: none;
  border: 1px solid transparent;
  border-radius: 999px;
  padding: 0 3.5rem 0 1rem;
  font-size: var(--text-lg);
  font-weight: 400;
  transition:
    border-color var(--transition-fast),
    background-color var(--transition-fast);
}

.searchInput::placeholder {
  color: var(--text-subdued);
}

.searchInput:focus {
  border-color: white;
  background: #282828;
  box-shadow: 0 0 0 1px white;
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

.searchGlyph {
  position: absolute;
  left: 16px;
  width: 24px;
  height: 24px;
  fill: none;
  stroke: var(--text-subdued);
  stroke-width: 2;
  pointer-events: none;
}
.searchInput {
  padding-left: 48px;
}
.searchPopover {
  position: absolute;
  top: calc(100% + 8px);
  left: 0;
  right: 0;
  z-index: var(--z-dropdown);
  max-height: min(68vh, 590px);
  overflow-y: auto;
  padding: 8px;
  background: #282828;
  border: 0;
  border-radius: 8px;
  box-shadow: var(--shadow-menu);
}
.popoverHeader {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  padding: 4px 8px 8px;
  color: var(--text-subdued);
  font-size: 12px;
}
.popoverState {
  margin: 0;
  padding: 20px 8px;
  color: var(--text-subdued);
  font-size: 14px;
}
.suggestionSectionTitle {
  padding: 8px;
  margin: 0;
  font-size: 16px;
  font-weight: 700;
  color: var(--text-base);
}
.suggestionRow {
  display: flex;
  align-items: center;
  gap: 12px;
  min-height: 64px;
  width: 100%;
  padding: 8px;
  border: 0;
  border-radius: 4px;
  background: transparent;
  color: var(--text-base);
  text-align: left;
  cursor: pointer;
}
.suggestionRow:hover,
.suggestionRow:focus-visible,
.recentSearchRow:hover {
  background: #ffffff1a;
}
.suggestionImage {
  width: 48px;
  height: 48px;
  flex: 0 0 48px;
  border-radius: 4px;
  object-fit: cover;
  background: var(--surface-raised);
}
.roundSuggestionImage {
  border-radius: 50%;
}
.suggestionText {
  display: flex;
  flex-direction: column;
  min-width: 0;
  gap: 2px;
}
.suggestionTitle,
.suggestionSubtitle {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.suggestionTitle {
  color: var(--text-base);
  font-size: 16px;
  line-height: 22px;
  font-weight: 400;
}
.suggestionSubtitle {
  color: var(--text-subdued);
  font-size: 14px;
  line-height: 20px;
}
.queryIcon {
  width: 48px;
  height: 48px;
  padding: 12px;
  flex: 0 0 48px;
  fill: none;
  stroke: var(--text-subdued);
  stroke-width: 1.8;
}
.recentSearchRow {
  display: flex;
  align-items: center;
  border-radius: 4px;
}
.recentSearchRow .suggestionRow {
  min-width: 0;
  flex: 1;
}
.removeRecent {
  display: grid;
  place-items: center;
  width: 32px;
  height: 32px;
  flex: 0 0 32px;
  margin-right: 8px;
  color: var(--text-subdued);
  border-radius: 50%;
}
.removeRecent svg {
  width: 16px;
  height: 16px;
  fill: none;
  stroke: currentColor;
  stroke-width: 2;
}
.removeRecent:hover {
  color: white;
  background: var(--surface-hover);
}
.searchPopover button:focus-visible,
#clearQueryButton:focus-visible {
  outline: 2px solid white;
  outline-offset: -2px;
}
.crossIcon {
  width: 24px;
  height: 24px;
  stroke: var(--text-subdued);
  stroke-width: 2;
  fill: none;
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

.userActions a:focus-visible {
  outline: 2px solid var(--spotify-green);
  outline-offset: 2px;
}
.userActions a:active {
  background: var(--surface-active);
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
@media (max-width: 767px) {
  .topBarContent {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    grid-template-rows: 40px 48px;
    gap: 8px;
    padding: 8px;
  }
  .userActions {
    justify-content: flex-end;
    gap: 0;
  }
  .searchInputContainer {
    grid-column: 1 / -1;
    grid-row: 2;
    max-width: none;
  }
}
</style>
