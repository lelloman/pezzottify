<template>
  <section class="showsView">
    <header class="showsHeader">
      <div>
        <h1>Shows</h1>
        <p v-if="!activeShow">Published long-form shows from the catalog.</p>
        <p v-else>{{ activeShow.summary }}</p>
      </div>
      <button v-if="activeShow" class="secondaryButton" @click="goToList">All Shows</button>
    </header>

    <div v-if="isLoading" class="mutedState">Loading...</div>

    <div v-else-if="!activeShow" class="showGrid">
      <button
        v-for="show in shows"
        :key="show.id"
        class="showCard"
        @click="openShow(show.id)"
      >
        <span class="showTitle">{{ show.title }}</span>
        <span class="showSummary">{{ show.summary }}</span>
        <span class="showMeta">{{ show.track_count }} tracks · {{ show.target_duration_minutes }} min</span>
      </button>
      <div v-if="shows.length === 0" class="mutedState">No published shows yet.</div>
    </div>

    <div v-else class="showPlayer">
      <div class="nowPlaying">
        <span class="eyebrow">{{ currentSegment?.kind || "ready" }}</span>
        <h2>{{ currentSegment?.title || activeShow.title }}</h2>
        <p>{{ currentNarration }}</p>
        <div class="controls">
          <button class="primaryButton" @click="togglePlayback">{{ isPlaying ? "Pause" : "Play" }}</button>
          <button class="secondaryButton" @click="previousSegment" :disabled="currentIndex === 0">Previous</button>
          <button class="secondaryButton" @click="nextSegment" :disabled="currentIndex >= playableSegments.length - 1">Next</button>
        </div>
      </div>

      <ol class="timeline">
        <li
          v-for="(segment, index) in playableSegments"
          :key="segment.id"
          :class="{ active: index === currentIndex }"
          @click="playSegment(index)"
        >
          <span class="segmentKind">{{ segment.kind }}</span>
          <span class="segmentTitle">{{ segment.title }}</span>
        </li>
      </ol>

      <audio ref="audioEl" :src="currentSrc" @ended="nextSegment" @play="isPlaying = true" @pause="isPlaying = false" controls />
    </div>
  </section>
</template>

<script setup>
import { computed, onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { useRemoteStore } from "@/store/remote";

const route = useRoute();
const router = useRouter();
const remote = useRemoteStore();

const shows = ref([]);
const activeShow = ref(null);
const isLoading = ref(false);
const currentIndex = ref(0);
const audioEl = ref(null);
const isPlaying = ref(false);

const playableSegments = computed(() => activeShow.value?.segments || []);
const currentSegment = computed(() => playableSegments.value[currentIndex.value] || null);
const currentNarration = computed(() => currentSegment.value?.text || "");
const currentSrc = computed(() => {
  const show = activeShow.value;
  const segment = currentSegment.value;
  if (!show || !segment) return "";
  if (segment.kind === "track" && segment.track_id) {
    return `/v1/content/stream/${encodeURIComponent(segment.track_id)}`;
  }
  return `/v1/content/show/${encodeURIComponent(show.id)}/segment/${encodeURIComponent(segment.id)}/stream`;
});

async function loadList() {
  isLoading.value = true;
  try {
    shows.value = await remote.fetchShows();
  } finally {
    isLoading.value = false;
  }
}

async function loadShow(id) {
  if (!id) {
    activeShow.value = null;
    await loadList();
    return;
  }
  isLoading.value = true;
  try {
    activeShow.value = await remote.fetchShow(id);
    currentIndex.value = 0;
  } finally {
    isLoading.value = false;
  }
}

function openShow(id) {
  router.push(`/show/${id}`);
}

function goToList() {
  router.push("/shows");
}

async function playSegment(index) {
  currentIndex.value = index;
  await Promise.resolve();
  audioEl.value?.play?.();
}

function togglePlayback() {
  if (!audioEl.value) return;
  if (audioEl.value.paused) audioEl.value.play();
  else audioEl.value.pause();
}

function nextSegment() {
  if (currentIndex.value >= playableSegments.value.length - 1) {
    isPlaying.value = false;
    return;
  }
  playSegment(currentIndex.value + 1);
}

function previousSegment() {
  if (currentIndex.value > 0) playSegment(currentIndex.value - 1);
}

watch(
  () => route.params.showId,
  (id) => loadShow(id),
  { immediate: true },
);

onMounted(() => {
  if (!route.params.showId) loadList();
});
</script>

<style scoped>
.showsView {
  min-height: 100%;
}

.showsHeader {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--spacing-4);
  margin-bottom: var(--spacing-6);
}

.showsHeader h1 {
  margin: 0 0 var(--spacing-2);
  font-size: var(--text-3xl);
}

.showsHeader p,
.showSummary,
.showMeta,
.mutedState,
.nowPlaying p {
  color: var(--text-subdued);
}

.showGrid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: var(--spacing-4);
}

.showCard {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  min-height: 160px;
  padding: var(--spacing-4);
  border: 1px solid var(--surface-border);
  border-radius: 8px;
  background: var(--bg-elevated-base);
  color: var(--text-base);
  text-align: left;
  cursor: pointer;
}

.showCard:hover {
  background: var(--bg-highlight);
}

.showTitle {
  font-weight: var(--font-bold);
  margin-bottom: var(--spacing-2);
}

.showSummary {
  flex: 1;
  line-height: 1.4;
}

.showMeta {
  margin-top: var(--spacing-3);
  font-size: var(--text-sm);
}

.showPlayer {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(260px, 360px);
  gap: var(--spacing-6);
}

.nowPlaying {
  min-width: 0;
}

.eyebrow,
.segmentKind {
  color: var(--spotify-green);
  font-size: var(--text-xs);
  font-weight: var(--font-bold);
  text-transform: uppercase;
}

.nowPlaying h2 {
  margin: var(--spacing-2) 0;
  font-size: var(--text-2xl);
}

.controls {
  display: flex;
  flex-wrap: wrap;
  gap: var(--spacing-2);
  margin: var(--spacing-4) 0;
}

.primaryButton,
.secondaryButton {
  border: 0;
  border-radius: 8px;
  padding: 10px 14px;
  font-weight: var(--font-bold);
  cursor: pointer;
}

.primaryButton {
  background: var(--spotify-green);
  color: #000;
}

.secondaryButton {
  background: var(--bg-elevated-base);
  color: var(--text-base);
  border: 1px solid var(--surface-border);
}

.secondaryButton:disabled {
  opacity: 0.45;
  cursor: default;
}

.timeline {
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: var(--spacing-2);
  margin: 0;
  padding: 0;
}

.timeline li {
  display: grid;
  grid-template-columns: 74px 1fr;
  gap: var(--spacing-3);
  align-items: center;
  padding: var(--spacing-3);
  border: 1px solid var(--surface-border);
  border-radius: 8px;
  cursor: pointer;
}

.timeline li.active {
  background: rgba(29, 185, 84, 0.12);
  border-color: var(--spotify-green);
}

.segmentTitle {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

audio {
  grid-column: 1 / -1;
  width: 100%;
}

@media (max-width: 900px) {
  .showPlayer {
    grid-template-columns: 1fr;
  }
}
</style>
