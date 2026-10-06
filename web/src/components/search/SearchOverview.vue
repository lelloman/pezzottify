<template>
  <div class="searchOverview" :aria-busy="loading">
    <div class="filters" role="group" aria-label="Filter search results">
      <button
        v-for="filter in filters"
        :key="filter.value"
        type="button"
        :aria-pressed="activeFilter === filter.value"
        @click="setFilter(filter.value)"
      >
        {{ filter.label }}
      </button>
    </div>
    <p v-if="error" class="state" role="alert">
      Search could not be completed. Please try again.
    </p>
    <p v-else-if="loading && !items.length" class="state" role="status">
      Searching…
    </p>
    <p v-else-if="!items.length && !loading" class="state" role="status">
      No matching music found.
    </p>
    <div v-if="activeFilter === 'all' && topResult" class="topGrid">
      <section>
        <h2>Top result</h2>
        <SearchEntityCard :result="topResult" hero />
      </section>
      <section v-if="songs.length">
        <div class="sectionHeader">
          <h2>Songs</h2>
          <button
            v-if="songs.length > 4"
            type="button"
            @click="setFilter('track')"
          >
            Show all
          </button>
        </div>
        <div v-for="song in songs.slice(0, 4)" :key="song.id" class="songRow">
          <QueueTrackRow
            :trackId="song.id"
            @play="playback.setTrack(song)"
            @menu="trackMenu?.openMenu($event, song.id)"
          />
          <span class="duration">{{ duration(song) }}</span>
        </div>
      </section>
    </div>
    <section
      v-if="
        activeFilter !== 'all' &&
        selectedTypes.includes('track') &&
        songs.length
      "
    >
      <h2>Songs</h2>
      <div v-for="song in songs" :key="song.id" class="songRow">
        <QueueTrackRow
          :trackId="song.id"
          @play="playback.setTrack(song)"
          @menu="trackMenu?.openMenu($event, song.id)"
        /><span class="duration">{{ duration(song) }}</span>
      </div>
    </section>
    <section v-for="group in groups" :key="group.type">
      <h2>{{ group.title }}</h2>
      <div class="cards">
        <SearchEntityCard
          v-for="result in group.items"
          :key="result.id"
          :result="result"
        />
      </div>
    </section>
    <p
      v-if="items.length && activeFilter !== 'all' && !filteredCount"
      class="state"
      role="status"
    >
      No matches for this filter.
    </p>
    <Teleport to="body"><TrackContextMenu ref="trackMenu" /></Teleport>
  </div>
</template>
<script setup>
import { computed, ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import { usePlaybackStore } from "@/store/playback";
import QueueTrackRow from "../common/QueueTrackRow.vue";
import TrackContextMenu from "../common/contextmenu/TrackContextMenu.vue";
import SearchEntityCard from "./SearchEntityCard.vue";
const props = defineProps({
  items: { type: Array, default: () => [] },
  primary: Object,
  loading: Boolean,
  error: Boolean,
});
const route = useRoute(),
  router = useRouter(),
  playback = usePlaybackStore(),
  trackMenu = ref(null);
const filters = [
  { value: "all", label: "All" },
  { value: "track", label: "Songs" },
  { value: "album", label: "Albums" },
  { value: "artist", label: "Artists" },
];
const selectedTypes = computed(() =>
  String(route.query.type || "track,album,artist")
    .split(",")
    .filter((t) => ["track", "album", "artist"].includes(t)),
);
const activeFilter = computed(() =>
  selectedTypes.value.length === 3 ? "all" : selectedTypes.value.join(","),
);
const setFilter = (value) => {
  const query = { ...route.query };
  if (value === "all") delete query.type;
  else query.type = value;
  router.replace({ query });
};
const topResult = computed(() => props.primary || props.items[0]);
const songs = computed(() =>
  props.items.filter((item) => item.type === "Track"),
);
const groups = computed(() =>
  [
    { type: "Album", title: "Albums" },
    { type: "Artist", title: "Artists" },
  ]
    .filter((group) => selectedTypes.value.includes(group.type.toLowerCase()))
    .map((group) => ({
      ...group,
      items: props.items.filter((item) => item.type === group.type),
    }))
    .filter((group) => group.items.length),
);
const filteredCount = computed(
  () =>
    props.items.filter((item) =>
      selectedTypes.value.includes(item.type.toLowerCase()),
    ).length,
);
const duration = (song) => {
  const seconds = Math.floor(
    song.duration_ms != null ? song.duration_ms / 1000 : song.duration || 0,
  );
  return seconds
    ? `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`
    : "";
};
</script>
<style scoped>
.searchOverview {
  display: flex;
  flex-direction: column;
  gap: 28px;
  min-width: 0;
}
.filters {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}
.filters button {
  padding: 6px 12px;
  min-height: 32px;
  border-radius: 999px;
  background: #ffffff12;
  font-size: 14px;
  color: white;
}
.filters button:hover {
  background: #ffffff24;
}
.filters button[aria-pressed="true"] {
  color: #121212;
  background: white;
}
h2 {
  margin: 0 0 16px;
  font-size: 24px;
  line-height: 1.2;
  font-weight: 700;
}
.topGrid {
  display: grid;
  grid-template-columns: minmax(0, 2fr) minmax(0, 3fr);
  gap: 24px;
}
.topGrid > section {
  min-width: 0;
}
.sectionHeader {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 8px;
}
.sectionHeader button {
  font-size: 14px;
  color: var(--text-subdued);
  white-space: nowrap;
}
.sectionHeader button:hover {
  text-decoration: underline;
}
.songRow {
  display: flex;
  align-items: center;
  min-width: 0;
}
.songRow :deep(.queueRow) {
  flex: 1;
  min-width: 0;
}
.duration {
  font-size: 14px;
  color: var(--text-subdued);
  padding: 0 8px;
}
.cards {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
  gap: 8px;
}
.state {
  color: var(--text-subdued);
  padding: 16px 0;
}
button:focus-visible {
  outline: 2px solid white;
  outline-offset: 3px;
}
@container (max-width:650px) {
  .topGrid {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
