<template>
  <div class="discographyContainer">
    <div class="header">
      <h2>{{ appearsOn ? "Appears In" : "Discography" }}</h2>
      <details
        ref="sortMenu"
        class="sortSelector"
        @keydown.esc.stop.prevent="closeSort(true)"
      >
        <summary
          :aria-label="appearsOn ? 'Sort appearances' : 'Sort discography'"
          title="Sort by"
        >
          {{ sortOrder === "popularity" ? "Popularity" : "Release date" }}
          <svg viewBox="0 0 24 24" aria-hidden="true">
            <path d="m6 9 6 6 6-6" />
          </svg>
        </summary>
        <div class="sortOptions" @keydown="navigateOptions">
          <p>Sort by</p>
          <button
            v-for="option in sortOptions"
            :key="option.value"
            type="button"
            :aria-pressed="sortOrder === option.value"
            :disabled="isLoading"
            @click="selectSort(option.value)"
          >
            {{ option.label
            }}<span v-if="sortOrder === option.value" aria-hidden="true"
              >✓</span
            >
          </button>
        </div>
      </details>
    </div>

    <div
      v-if="albums.length > 0"
      class="albumsContainer"
      ref="albumsContainerRef"
    >
      <SearchEntityCard
        v-for="album in albums"
        :key="album.id"
        :result="{ ...album, type: 'Album' }"
        :metaLabel="albumSubtitle(album)"
        @contextmenu.prevent="
          entityMenu?.openMenu($event, 'album', album.id, album.name)
        "
      />
    </div>

    <div v-if="isLoading" class="loadingIndicator">
      <div class="spinner"></div>
    </div>

    <div v-if="error" class="error">{{ error }}</div>

    <div ref="sentinelRef" class="sentinel"></div>
    <Teleport to="body"><EntityContextMenu ref="entityMenu" /></Teleport>
  </div>
</template>

<script setup>
import { onMounted, onUnmounted, watch, ref } from "vue";
import SearchEntityCard from "@/components/search/SearchEntityCard.vue";
import EntityContextMenu from "./contextmenu/EntityContextMenu.vue";
import { useRemoteStore } from "@/store/remote";

const props = defineProps({
  artistId: {
    type: String,
    required: true,
  },
  appearsOn: {
    type: Boolean,
    default: false,
  },
});

const entityMenu = ref(null);
const albumSubtitle = (album) => {
  const year =
    album.year || album.release_year || album.release_date?.slice(0, 4);
  const kind =
    { single: "Single", ep: "EP", compilation: "Compilation", album: "Album" }[
      album.album_type?.toLowerCase()
    ] || "Album";
  return [year, kind].filter(Boolean).join(" · ");
};
const PAGE_SIZE = 50;

const remoteStore = useRemoteStore();
const albums = ref([]);
const hasMore = ref(true);
const error = ref(null);
const isLoading = ref(false);
const sortOrder = ref("popularity");
const offset = ref(0);
const sortMenu = ref(null);
const sortOptions = [
  { value: "popularity", label: "Popularity" },
  { value: "release_date", label: "Release date" },
];
const closeSort = (restoreFocus = false) => {
  if (!sortMenu.value) return;
  sortMenu.value.open = false;
  if (restoreFocus) sortMenu.value.querySelector("summary")?.focus();
};
const outsideSort = (event) => {
  if (!sortMenu.value?.contains(event.target)) closeSort();
};
const selectSort = (value) => {
  closeSort(true);
  if (sortOrder.value === value) return;
  sortOrder.value = value;
  resetAndLoad();
};
const navigateOptions = (event) => {
  if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) return;
  const buttons = [...sortMenu.value.querySelectorAll("button:not(:disabled)")];
  const index = buttons.indexOf(document.activeElement);
  const next =
    event.key === "Home"
      ? 0
      : event.key === "End"
        ? buttons.length - 1
        : (index + (event.key === "ArrowDown" ? 1 : -1) + buttons.length) %
          buttons.length;
  event.preventDefault();
  buttons[next]?.focus();
};

const sentinelRef = ref(null);
let observer = null;

const loadMore = async () => {
  if (isLoading.value || !hasMore.value) return;

  isLoading.value = true;
  error.value = null;

  try {
    const response = await remoteStore.fetchArtistDiscography(props.artistId, {
      limit: PAGE_SIZE,
      offset: offset.value,
      sort: sortOrder.value,
      appears_on: props.appearsOn,
    });

    if (response) {
      albums.value = [...albums.value, ...response.albums];
      hasMore.value = response.has_more;
      offset.value += response.albums.length;
    } else {
      error.value = "Error fetching artist albums";
    }
  } catch {
    error.value = "Error fetching artist albums";
  }

  isLoading.value = false;
};

const resetAndLoad = () => {
  albums.value = [];
  offset.value = 0;
  hasMore.value = true;
  error.value = null;
  loadMore();
};

const setupIntersectionObserver = () => {
  if (observer) {
    observer.disconnect();
  }

  observer = new IntersectionObserver(
    (entries) => {
      if (entries[0].isIntersecting && !isLoading.value && hasMore.value) {
        loadMore();
      }
    },
    {
      rootMargin: "600px",
    },
  );

  if (sentinelRef.value) {
    observer.observe(sentinelRef.value);
  }
};

watch(
  () => props.artistId,
  () => {
    resetAndLoad();
  },
);

onMounted(() => {
  document.addEventListener("pointerdown", outsideSort);
  loadMore();
  setupIntersectionObserver();
});

onUnmounted(() => {
  document.removeEventListener("pointerdown", outsideSort);
  if (observer) {
    observer.disconnect();
  }
});
</script>

<style scoped>
.discographyContainer {
  display: flex;
  flex-direction: column;
}

.header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;
}

.header h2 {
  margin: 0;
}

.sortSelector {
  position: relative;
  flex-shrink: 0;
  font-size: 14px;
}
.sortSelector summary {
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 32px;
  padding: 4px 0 4px 8px;
  list-style: none;
  color: var(--text-subdued);
  cursor: pointer;
}
.sortSelector summary::-webkit-details-marker {
  display: none;
}
.sortSelector summary:hover,
.sortSelector[open] summary {
  color: var(--text-base);
}
.sortSelector summary svg {
  width: 16px;
  height: 16px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.8;
}
.sortOptions {
  position: absolute;
  z-index: 40;
  top: calc(100% + 4px);
  right: 0;
  width: 200px;
  padding: 4px;
  border-radius: 4px;
  background: var(--menu-background);
  box-shadow: var(--shadow-menu);
}
.sortOptions p {
  padding: 12px;
  margin: 0;
  color: var(--text-subdued);
  font-size: 12px;
  font-weight: 700;
}
.sortOptions button {
  width: 100%;
  min-height: 40px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 16px;
  padding: 8px 12px;
  border-radius: 2px;
  color: var(--text-base);
  text-align: left;
  font-size: 14px;
}
.sortOptions button:hover:not(:disabled) {
  background: var(--surface-hover);
}
.sortOptions button[aria-pressed="true"] {
  color: var(--spotify-green);
}
.sortOptions button:disabled {
  opacity: 0.5;
  cursor: wait;
}
.sortSelector summary:focus-visible,
.sortOptions button:focus-visible {
  outline: 2px solid white;
  outline-offset: 2px;
}
.header {
  gap: 12px;
  flex-wrap: wrap;
}
.header h2 {
  font-size: 24px;
  font-weight: 700;
}

.albumsContainer {
  display: grid;
  gap: 8px;
  grid-template-columns: repeat(auto-fill, minmax(min(170px, 100%), 1fr));
  margin-inline: -12px;
}

.loadingIndicator {
  display: flex;
  justify-content: center;
  padding: 24px;
}

.spinner {
  width: 32px;
  height: 32px;
  border: 3px solid var(--border-default, #333);
  border-top-color: var(--spotify-green, #1db954);
  border-radius: 50%;
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.error {
  text-align: center;
  padding: 16px;
  color: var(--error, #e91429);
}

.sentinel {
  height: 1px;
}
</style>
