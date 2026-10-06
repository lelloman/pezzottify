<template>
  <aside
    class="panel libraryPanel"
    :class="{ collapsed, expanded: layout === 'wide' }"
    aria-label="Your library"
  >
    <header class="libraryHeader">
      <button
        type="button"
        class="headingButton"
        :aria-label="
          collapsed ? 'Expand your library' : 'Collapse your library'
        "
        :title="collapsed ? 'Expand your library' : 'Collapse your library'"
        :aria-expanded="!collapsed"
        @click="setLayout(collapsed ? 'normal' : 'collapsed')"
      >
        <svg viewBox="0 0 24 24" aria-hidden="true">
          <path d="M4 4v16M9 4v16M15 4l5 15" />
        </svg>
        <h2 v-if="!collapsed">Your library</h2>
      </button>
      <template v-if="!collapsed">
        <button
          type="button"
          class="createButton"
          aria-label="Create playlist"
          title="Create playlist"
          :disabled="isCreatingPlaylist"
          @click="createPlaylist"
        >
          <PlusIcon /><span>Create</span>
        </button>
        <button
          type="button"
          class="iconButton expandButton"
          :aria-label="
            layout === 'wide' ? 'Minimize your library' : 'Expand your library'
          "
          :title="
            layout === 'wide' ? 'Minimize your library' : 'Expand your library'
          "
          @click="setLayout(layout === 'wide' ? 'normal' : 'wide')"
        >
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path
              :d="
                layout === 'wide'
                  ? 'M20 4l-6 6m0-6v6h6M4 20l6-6m-6 0h6v6'
                  : 'M14 4h6v6M20 4l-6 6M10 20H4v-6M4 20l6-6'
              "
            />
          </svg>
        </button>
      </template>
    </header>
    <div v-if="!collapsed" class="libraryToolbar">
      <div class="filterRail">
        <div
          ref="filterScroller"
          class="filterChips"
          role="group"
          aria-label="Filter your library"
          @scroll="updateFilterOverflow"
        >
          <button
            v-if="filter"
            type="button"
            class="clearFilter iconButton"
            aria-label="Show all library items"
            title="Show all library items"
            @click="setFilter('')"
          >
            ×
          </button>
          <button
            v-for="item in filters"
            :key="item.type"
            type="button"
            class="filterChip"
            :class="{ active: filter === item.type }"
            :aria-pressed="filter === item.type"
            @click="setFilter(filter === item.type ? '' : item.type)"
          >
            {{ item.label }}
          </button>
        </div>
        <button
          v-if="canScrollLeft"
          class="filterArrow left iconButton"
          aria-label="Scroll library filters left"
          @click="scrollFilters(-1)"
        >
          <svg viewBox="0 0 24 24"><path d="m14 6-6 6 6 6" /></svg>
        </button>
        <button
          v-if="canScrollRight"
          class="filterArrow right iconButton"
          aria-label="Scroll library filters right"
          @click="scrollFilters(1)"
        >
          <svg viewBox="0 0 24 24"><path d="m10 6 6 6-6 6" /></svg>
        </button>
      </div>
      <div class="libraryTools">
        <div
          class="librarySearch"
          :class="{ open: searchOpen || layout === 'wide' }"
        >
          <button
            v-if="!searchOpen && layout !== 'wide'"
            type="button"
            class="iconButton"
            aria-label="Search your library"
            title="Search your library"
            @click="openSearch"
          >
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <circle cx="10" cy="10" r="6.5" />
              <path d="m15 15 5 5" />
            </svg>
          </button>
          <input
            v-else
            ref="searchInput"
            v-model="query"
            type="search"
            aria-label="Search your library"
            placeholder="Search your library"
            @keydown.esc.stop="closeSearch"
          />
        </div>
        <details ref="sortMenu" class="sortMenu" @keydown.esc.stop="closeSort">
          <summary aria-label="Sort your library" title="Sort your library">
            {{ sortLabel
            }}<svg viewBox="0 0 24 24" aria-hidden="true">
              <path
                :d="
                  layout === 'wide'
                    ? 'M3 3h6v6H3zM15 3h6v6h-6zM3 15h6v6H3zM15 15h6v6h-6z'
                    : 'M8 6h12M8 12h12M8 18h12M3 6h1M3 12h1M3 18h1'
                "
              />
            </svg>
          </summary>
          <div class="sortOptions">
            <p>Sort by</p>
            <button
              v-for="option in sortOptions"
              :key="option.value"
              type="button"
              :aria-pressed="sort === option.value"
              @click="setSort(option.value)"
            >
              {{ option.label
              }}<span v-if="sort === option.value" aria-hidden="true">✓</span>
            </button>
          </div>
        </details>
      </div>
    </div>
    <p v-if="actionError" class="actionError" role="alert">{{ actionError }}</p>
    <div
      ref="scrollContainer"
      class="libraryContent"
      :aria-busy="userStore.isInitializing"
      @keydown="navigateRows"
    >
      <p v-if="userStore.isInitializing" class="libraryState" role="status">
        Loading your library…
      </p>
      <ul
        v-else-if="visibleEntries.length"
        class="libraryList"
        aria-label="Saved items"
      >
        <LibraryRow
          v-for="entry in visibleEntries"
          :key="entry.key"
          :entry="entry"
          :selected="isSelected(entry)"
          :playing="isPlaying(entry)"
          :collapsed="collapsed"
          :expanded="layout === 'wide'"
          @navigate="leaveExpanded"
          :busy="busyKey === entry.key"
          @play="playEntry"
          @contextmenu="openContextMenu($event, entry)"
        />
      </ul>
      <p v-else class="libraryState" role="status">
        {{
          pending
            ? "Searching saved items…"
            : query
              ? "No matches in your library."
              : filter
                ? `No saved ${filter}s yet.`
                : "Your library is empty. Save albums and artists, or create a playlist."
        }}
      </p>
    </div>
    <Teleport to="body"><EntityContextMenu ref="entityMenu" /></Teleport>
  </aside>
</template>
<script setup>
import {
  computed,
  ref,
  watch,
  nextTick,
  onMounted,
  onBeforeUnmount,
} from "vue";
import { useRoute, useRouter } from "vue-router";
import { useUserStore } from "@/store/user";
import { useStaticsStore } from "@/store/statics";
import { usePlaybackStore } from "@/store/playback";
import { chooseAlbumCoverImageUrl, chooseSmallArtistImageUrl } from "@/utils";
import LibraryRow from "./common/LibraryRow.vue";
import PlusIcon from "./icons/PlusIcon.vue";
import EntityContextMenu from "./common/contextmenu/EntityContextMenu.vue";
const props = defineProps({ requestedLayout: String });
const emit = defineEmits(["layout-change"]);
const userStore = useUserStore(),
  statics = useStaticsStore(),
  playback = usePlaybackStore(),
  route = useRoute(),
  router = useRouter();
const filters = [
  { type: "playlist", label: "Playlists" },
  { type: "artist", label: "Artists" },
  { type: "album", label: "Albums" },
];
const sortOptions = [
  { value: "name", label: "Alphabetical" },
  { value: "creator", label: "Creator" },
  { value: "played", label: "Played this session" },
];
const stored = (key, allowed, fallback) => {
  try {
    const value = localStorage.getItem(key);
    return allowed.includes(value) ? value : fallback;
  } catch {
    return fallback;
  }
};
const filter = ref(
  stored("library.filter", ["", "album", "artist", "playlist"], ""),
);
const sort = ref(
  stored(
    "library.sort",
    sortOptions.map((o) => o.value),
    "name",
  ),
);
const layout = ref(
  stored("library.layout", ["normal", "wide", "collapsed"], "normal"),
);
const collapsed = computed(() => layout.value === "collapsed");
const query = ref(""),
  searchOpen = ref(false),
  searchInput = ref(null),
  sortMenu = ref(null),
  scrollContainer = ref(null),
  entityMenu = ref(null),
  isCreatingPlaylist = ref(false),
  actionError = ref(""),
  busyKey = ref(null);
const played = ref({});
const refs = new Map();
// Cache references once: statics getters retry empty entries, so avoid invoking them on each filter pass.
const data = (type, id) => {
  const key = `${type}:${id}`;
  if (!refs.has(key))
    refs.set(
      key,
      type === "album"
        ? statics.getAlbum(id)
        : type === "artist"
          ? statics.getArtist(id)
          : statics.getTrack(id),
    );
  return refs.get(key);
};
const artistNames = (ids) =>
  (ids || [])
    .map((id) => data("artist", id).item?.name)
    .filter(Boolean)
    .join(", ");
const playlistImages = (playlist) => {
  const tracks = playlist.tracks || [];
  const albums = [
    ...new Set(
      Array.from(
        { length: Math.min(4, tracks.length) },
        (_, i) =>
          data(
            "track",
            tracks[
              Math.floor((i * tracks.length) / Math.min(4, tracks.length))
            ],
          ).item?.album_id,
      ).filter(Boolean),
    ),
  ];
  const images = albums.map((id) => chooseAlbumCoverImageUrl({ id }));
  return images.length > 1
    ? Array.from({ length: 4 }, (_, i) => images[i % images.length])
    : images;
};
const entries = computed(() => [
  ...(userStore.likedAlbumIds || []).map((id) => {
    const ref = data("album", id),
      item = ref.item,
      creator = artistNames(item?.artists_ids);
    return {
      key: `album:${id}`,
      type: "album",
      id,
      name: item?.name || (ref.error ? "Album unavailable" : "Loading album…"),
      creator,
      subtitle: creator ? `Album · ${creator}` : "Album",
      images: [chooseAlbumCoverImageUrl({ id })],
      ready: !!item,
      pending: !item && !ref.error,
    };
  }),
  ...(userStore.likedArtistsIds || []).map((id) => {
    const ref = data("artist", id),
      item = ref.item;
    return {
      key: `artist:${id}`,
      type: "artist",
      id,
      name:
        item?.name || (ref.error ? "Artist unavailable" : "Loading artist…"),
      creator: item?.name || "",
      subtitle: "Artist",
      images: [chooseSmallArtistImageUrl({ id })],
      ready: !!item,
      pending: !item && !ref.error,
    };
  }),
  ...(userStore.playlistsData?.list || []).map((item) => ({
    key: `playlist:${item.id}`,
    type: "playlist",
    id: item.id,
    name: item.name,
    creator: "",
    subtitle: `Playlist · ${item.tracks?.length || 0} tracks`,
    images: playlistImages(item),
    ready: true,
    pending: false,
    item,
  })),
]);
const pending = computed(() => entries.value.some((e) => e.pending));
const normalize = (value) =>
  value.normalize("NFD").replace(/\p{M}/gu, "").toLocaleLowerCase();
const visibleEntries = computed(() => {
  const text = normalize(query.value.trim());
  return entries.value
    .filter(
      (e) =>
        (!filter.value || e.type === filter.value) &&
        (!text ||
          normalize(`${e.name} ${e.creator} ${e.subtitle}`).includes(text)),
    )
    .sort((a, b) => {
      if (sort.value === "played") {
        const delta = (played.value[b.key] || 0) - (played.value[a.key] || 0);
        if (delta) return delta;
      }
      if (sort.value === "creator") {
        const delta = a.creator.localeCompare(b.creator, undefined, {
          sensitivity: "base",
        });
        if (delta) return delta;
      }
      return a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
    });
});
const sortLabel = computed(
  () => sortOptions.find((o) => o.value === sort.value)?.label,
);
const isSelected = (entry) =>
  route.name === entry.type && route.params[`${entry.type}Id`] === entry.id;
const playbackKey = computed(() => {
  const list = playback.currentPlaylist;
  if (list?.type === playback.PLAYBACK_CONTEXTS.userPlaylist)
    return `playlist:${list.context.id}`;
  if (list?.type === playback.PLAYBACK_CONTEXTS.album)
    return `album:${list.context.id}`;
  if (
    list?.type === playback.PLAYBACK_CONTEXTS.radio &&
    list.context?.seed?.entity_type === "artist"
  )
    return `artist:${list.context.seed.entity_id}`;
  return null;
});
const isPlaying = (entry) =>
  playback.isPlaying && entry.key === playbackKey.value;
watch(
  [playbackKey, () => playback.isPlaying],
  ([key, active]) => {
    if (key && active) played.value = { ...played.value, [key]: Date.now() };
  },
  { immediate: true },
);
const persist = (key, value) => {
  try {
    localStorage.setItem(key, value);
  } catch {
    /* Preferences can remain in memory. */
  }
};
const setFilter = (value) => {
  filter.value = value;
  persist("library.filter", value);
};
const setLayout = (value) => {
  layout.value = value;
  persist("library.layout", value);
  emit("layout-change", value);
};
watch(
  () => props.requestedLayout,
  (value) => {
    if (value && value !== layout.value) setLayout(value);
  },
);
const leaveExpanded = (event) => {
  if (
    layout.value === "wide" &&
    !event.ctrlKey &&
    !event.metaKey &&
    !event.shiftKey &&
    event.button === 0
  )
    setLayout("normal");
};
const filterScroller = ref(null);
const canScrollLeft = ref(false),
  canScrollRight = ref(false);
const updateFilterOverflow = () => {
  const el = filterScroller.value;
  canScrollLeft.value = !!el && el.scrollLeft > 1;
  canScrollRight.value =
    !!el && el.scrollLeft + el.clientWidth < el.scrollWidth - 1;
};
const scrollFilters = (direction) =>
  filterScroller.value?.scrollBy({
    left: direction * 140,
    behavior: matchMedia("(prefers-reduced-motion: reduce)").matches
      ? "instant"
      : "smooth",
  });
const filterObserver = new ResizeObserver(updateFilterOverflow);
watch(
  filterScroller,
  (el, old) => {
    if (old) filterObserver.unobserve(old);
    if (el) filterObserver.observe(el);
    updateFilterOverflow();
  },
  { flush: "post" },
);
watch(filter, () => nextTick(updateFilterOverflow));
onBeforeUnmount(() => filterObserver.disconnect());
const closeSort = (event) => {
  if (sortMenu.value) {
    sortMenu.value.open = false;
    if (event?.type === "keydown")
      sortMenu.value.querySelector("summary")?.focus();
  }
};
const setSort = (value) => {
  sort.value = value;
  persist("library.sort", value);
  closeSort();
  sortMenu.value?.querySelector("summary")?.focus();
};
const openSearch = async () => {
  searchOpen.value = true;
  await nextTick();
  searchInput.value?.focus();
};
const closeSearch = () => {
  query.value = "";
  searchOpen.value = false;
};
const outside = (event) => {
  if (sortMenu.value && !sortMenu.value.contains(event.target)) closeSort();
};
watch([filter, query, sort], () => {
  if (scrollContainer.value) scrollContainer.value.scrollTop = 0;
});
const navigateRows = (event) => {
  if (
    !event.target.closest(".libraryLink") ||
    ![
      "ArrowDown",
      "ArrowUp",
      "ArrowLeft",
      "ArrowRight",
      "Home",
      "End",
    ].includes(event.key)
  )
    return;
  const links = [...scrollContainer.value.querySelectorAll(".libraryLink")];
  const index = links.indexOf(event.target.closest(".libraryLink"));
  const columns =
    layout.value === "wide"
      ? getComputedStyle(
          scrollContainer.value.querySelector(".libraryList"),
        ).gridTemplateColumns.split(" ").length
      : 1;
  const step = {
    ArrowDown: columns,
    ArrowUp: -columns,
    ArrowLeft: -1,
    ArrowRight: 1,
  }[event.key];
  const next =
    event.key === "Home"
      ? 0
      : event.key === "End"
        ? links.length - 1
        : Math.max(0, Math.min(links.length - 1, index + step));
  event.preventDefault();
  links[next]?.focus();
};
const openContextMenu = (event, entry) => {
  if (entry.type !== "playlist") {
    event.preventDefault();
    entityMenu.value?.openMenu(event, entry.type, entry.id, entry.name);
  }
};
const playEntry = async (entry) => {
  if (busyKey.value) return;
  busyKey.value = entry.key;
  actionError.value = "";
  try {
    if (entry.type === "album") await playback.setAlbumId(entry.id);
    else if (entry.type === "artist")
      await playback.setArtistGreatestHits(entry.id);
    else {
      await userStore.loadPlaylistData(entry.id);
      const item = userStore.playlistsData?.by_id?.[entry.id];
      if (!item) throw Error("Playlist unavailable");
      if (!item.tracks?.length) {
        actionError.value = "This playlist has no tracks yet.";
        return;
      }
      await playback.setUserPlaylist(item);
    }
    if (playback.isPlaying && playbackKey.value === entry.key) {
      played.value = { ...played.value, [entry.key]: Date.now() };
    }
  } catch {
    actionError.value = "Could not start playback. Please try again.";
  } finally {
    busyKey.value = null;
  }
};
const createPlaylist = async () => {
  if (isCreatingPlaylist.value) return;
  isCreatingPlaylist.value = true;
  actionError.value = "";
  try {
    await userStore.createPlaylist((result) => {
      const id = typeof result === "string" ? result : result?.id;
      if (id) {
        if (layout.value === "wide") setLayout("normal");
        setFilter("playlist");
        query.value = "";
        router.push({
          name: "playlist",
          params: { playlistId: id },
          query: { edit: "true" },
        });
      } else
        actionError.value = "Could not create the playlist. Please try again.";
    });
  } catch {
    actionError.value = "Could not create the playlist. Please try again.";
  } finally {
    isCreatingPlaylist.value = false;
  }
};
onMounted(() => {
  emit("layout-change", layout.value);
  document.addEventListener("pointerdown", outside);
});
onBeforeUnmount(() => document.removeEventListener("pointerdown", outside));
</script>
<style scoped>
.libraryPanel {
  border: 0;
  min-height: 0;
  min-width: 0;
  overflow: hidden;
  container-type: inline-size;
}
.libraryHeader {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 16px 12px 8px;
  flex-shrink: 0;
}
.headingButton {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  flex: 1;
  text-align: left;
  height: 36px;
}
.headingButton h2 {
  font-size: 16px;
  font-weight: 700;
  white-space: nowrap;
}
.headingButton svg {
  display: none;
  width: 20px;
  height: 20px;
  fill: none;
  stroke: currentColor;
  stroke-width: 2;
}
.headingButton:hover {
  color: white;
}
.createButton {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  height: 36px;
  padding: 8px 12px;
  background: #1f1f1f;
  border-radius: 999px;
  font-size: 14px;
  font-weight: 700;
}
.createButton svg {
  width: 16px;
  height: 16px;
  fill: currentColor;
}
.createButton:hover {
  background: #2a2a2a;
}
.createButton:disabled {
  opacity: 0.5;
  cursor: wait;
}
.iconButton {
  display: grid;
  place-items: center;
  width: 32px;
  height: 32px;
  flex-shrink: 0;
  border-radius: 50%;
  color: var(--text-subdued);
}
.iconButton:hover {
  color: white;
  background: #ffffff1a;
}
.iconButton svg,
.sortMenu svg {
  width: 16px;
  height: 16px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.8;
}
.filterChips {
  display: flex;
  align-items: center;
  gap: 8px;
  overflow-x: auto;
  padding: 8px 12px;
  flex-shrink: 0;
}
.libraryToolbar {
  flex-shrink: 0;
  min-width: 0;
}
.filterRail {
  position: relative;
  min-width: 0;
}
.filterChips {
  scrollbar-width: none;
}
.filterChips::-webkit-scrollbar {
  display: none;
}
.filterArrow {
  position: absolute;
  top: 8px;
  background: #282828;
  color: white;
  z-index: 1;
}
.filterArrow.left {
  left: 4px;
  box-shadow: 8px 0 12px 4px #121212;
}
.filterArrow.right {
  right: 4px;
  box-shadow: -8px 0 12px 4px #121212;
}
.expanded .libraryToolbar {
  display: flex;
  align-items: center;
  gap: 16px;
  padding-right: 16px;
}
.expanded .filterRail {
  flex: 1;
}
.expanded .libraryTools {
  padding: 0;
  width: 250px;
}
.expanded .libraryList {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
  align-items: start;
}
.expanded .libraryHeader {
  padding-inline: 24px;
}
.filterChip {
  flex-shrink: 0;
  height: 32px;
  padding: 0 12px;
  border-radius: 999px;
  background: #ffffff12;
  color: white;
  font-size: 14px;
  white-space: nowrap;
}
.filterChip:hover {
  background: #ffffff24;
}
.filterChip.active {
  background: white;
  color: #121212;
}
.clearFilter {
  font-size: 24px;
}
.libraryTools {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 0 16px 8px;
  flex-shrink: 0;
}
.librarySearch {
  min-width: 0;
}
.librarySearch.open {
  flex: 1;
}
.librarySearch input {
  width: 100%;
  min-width: 0;
  height: 32px;
  border: 0;
  border-radius: 4px;
  padding: 6px 8px;
  background: #ffffff1a;
  color: white;
  font-size: 14px;
}
.sortMenu {
  position: relative;
  flex-shrink: 0;
}
.sortMenu summary {
  display: flex;
  align-items: center;
  gap: 6px;
  max-width: 140px;
  height: 32px;
  font-size: 14px;
  list-style: none;
  color: var(--text-subdued);
  cursor: pointer;
}
.sortMenu summary::-webkit-details-marker {
  display: none;
}
.sortMenu summary:hover {
  color: white;
}
.sortOptions {
  position: absolute;
  right: 0;
  top: 100%;
  width: 205px;
  padding: 4px;
  z-index: 50;
  background: var(--menu-background);
  border-radius: 4px;
  box-shadow: var(--shadow-menu);
}
.sortOptions p {
  padding: 12px;
  font-size: 12px;
  color: var(--text-subdued);
}
.sortOptions button {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  min-height: 40px;
  padding: 8px 12px;
  font-size: 14px;
  text-align: left;
  border-radius: 2px;
}
.sortOptions button:hover {
  background: #ffffff1a;
}
.sortOptions button[aria-pressed="true"] {
  color: var(--spotify-green);
}
.libraryContent {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 0 8px 8px;
}
.libraryList {
  margin: 0;
  padding: 0;
  list-style: none;
}
.libraryState {
  padding: 24px 12px;
  color: var(--text-subdued);
  font-size: 14px;
  line-height: 1.6;
  text-align: center;
}
.actionError {
  margin: 8px 16px;
  color: var(--text-subdued);
  font-size: 14px;
}
button:focus-visible,
summary:focus-visible,
input:focus-visible {
  outline: 2px solid white;
  outline-offset: 2px;
}
.collapsed .libraryHeader {
  padding: 16px 8px 8px;
  justify-content: center;
}
.collapsed .headingButton {
  flex: 0 0 48px;
  justify-content: center;
}
.collapsed .headingButton svg {
  display: block;
}
.collapsed .libraryContent {
  padding: 0 4px 8px;
}
@container (max-width:300px) {
  .createButton {
    padding: 8px;
    width: 36px;
  }
  .createButton span {
    display: none;
  }
  .libraryTools {
    padding-inline: 12px;
  }
}
.libraryTools:has(.librarySearch.open) .sortMenu summary {
  font-size: 0;
  width: 32px;
  justify-content: center;
}
</style>
