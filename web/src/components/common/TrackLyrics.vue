<template>
  <section class="lyricsPanel" :class="{ compact }" aria-label="Lyrics">
    <header>
      <h2>Lyrics</h2>
      <span v-if="lyrics?.status === 'found'" class="source"
        >LRCLIB · {{ lines.length ? "Synced lyrics" : "Plain lyrics" }}</span
      >
    </header>
    <p v-if="loading" role="status">Loading lyrics…</p>
    <template
      v-else-if="
        lyrics?.status === 'found' && (lines.length || lyrics.plain_lyrics)
      "
    >
      <div
        v-if="lines.length"
        class="timedLyrics"
        :class="{ expanded: compact && expanded }"
      >
        <template v-for="(line, index) in visibleLines" :key="index">
          <button
            v-if="canSeek"
            type="button"
            :class="{ active: index === activeIndex }"
            :aria-current="index === activeIndex ? 'true' : undefined"
            :aria-label="`Seek to ${Math.floor(line.time / 60)}:${String(Math.floor(line.time % 60)).padStart(2, '0')}: ${line.text || 'Instrumental break'}`"
            @click="seek(line.time)"
          >
            {{ line.text || "♪" }}
          </button>
          <p v-else :class="{ active: index === activeIndex }">
            {{ line.text || "♪" }}
          </p>
        </template>
      </div>
      <p v-else class="plainLyrics" :class="{ expanded: compact && expanded }">
        {{ visiblePlainLyrics }}
      </p>
      <button
        v-if="compact && hasMore"
        type="button"
        class="lyricsToggle"
        :aria-expanded="expanded"
        @click="expanded = !expanded"
      >
        {{ expanded ? "Show less" : "Show more" }}
      </button>
    </template>
    <p v-else-if="lyrics?.status === 'instrumental'">
      This track is instrumental.
    </p>
    <template v-else>
      <p role="status">{{ message }}</p>
      <button type="button" :disabled="requesting" @click="requestLyrics">
        {{ requesting ? "Fetching lyrics…" : "Find lyrics" }}
      </button>
      <button type="button" :disabled="requesting" @click="refresh">
        Check again
      </button>
    </template>
  </section>
</template>

<script setup>
import { computed, onUnmounted, ref, watch } from "vue";
import { useRemoteStore } from "@/store/remote";
import { usePlaybackStore } from "@/store/playback";
import { activeLyricIndex, parseSyncedLyrics } from "@/utils/lyrics";

const props = defineProps({
  trackId: { type: String, required: true },
  compact: Boolean,
});
const expanded = ref(false);
const remote = useRemoteStore();
const playback = usePlaybackStore();
const lyrics = ref(null);
const loading = ref(false);
const requesting = ref(false);
const notice = ref("");
let generation = 0;
let timer;
let controller;
const lines = computed(() => parseSyncedLyrics(lyrics.value?.synced_lyrics));
const plainLines = computed(() =>
  (lyrics.value?.plain_lyrics || "").split("\n"),
);
const hasMore = computed(
  () => (lines.value.length || plainLines.value.length) > 4,
);
const visibleLines = computed(() =>
  props.compact && !expanded.value ? lines.value.slice(0, 4) : lines.value,
);
const visiblePlainLyrics = computed(() =>
  (props.compact && !expanded.value
    ? plainLines.value.slice(0, 4)
    : plainLines.value
  ).join("\n"),
);
const isCurrent = computed(() => playback.currentTrackId === props.trackId);
const canSeek = computed(
  () => isCurrent.value && playback.currentTrack?.duration > 0,
);
const activeIndex = computed(() =>
  isCurrent.value ? activeLyricIndex(lines.value, playback.progressSec) : -1,
);
const message = computed(
  () =>
    notice.value ||
    {
      not_found:
        "No lyrics were found for this recording. You can try another search.",
      error: "The lyrics provider could not be reached. Try again later.",
    }[lyrics.value?.status] ||
    "No lyrics downloaded for this track yet.",
);

function seek(seconds) {
  if (canSeek.value)
    playback.seekToPercentage(
      Math.min(1, (seconds * 1000) / playback.currentTrack.duration),
    );
}

async function read(id, version) {
  controller?.abort();
  controller = new AbortController();
  const result = await remote.getTrackLyrics(id, controller.signal);
  if (version !== generation) return null;
  lyrics.value = result;
  return result;
}

async function refresh() {
  const version = generation;
  loading.value = true;
  notice.value = "";
  try {
    await read(props.trackId, version);
  } catch {
    if (version === generation)
      notice.value = "Could not load lyrics. Check again to retry.";
  } finally {
    if (version === generation) loading.value = false;
  }
}

async function requestLyrics() {
  const version = generation;
  const id = props.trackId;
  const previousFetched = lyrics.value?.fetched_at;
  requesting.value = true;
  notice.value = "Fetching lyrics…";
  try {
    await remote.downloadLyrics("track", id);
    if (version !== generation) return;
    let polls = 0;
    const poll = async () => {
      try {
        const result = await read(id, version);
        if (version !== generation) return;
        if (
          result &&
          (result.fetched_at !== previousFetched ||
            ["found", "instrumental"].includes(result.status))
        ) {
          requesting.value = false;
          notice.value = "";
          return;
        }
        if (++polls >= 30) {
          requesting.value = false;
          notice.value = "Lyrics are not ready yet. Check again shortly.";
          return;
        }
        timer = setTimeout(poll, 2000);
      } catch {
        if (version !== generation) return;
        requesting.value = false;
        notice.value = "Could not check the download. Check again to retry.";
      }
    };
    await poll();
  } catch (error) {
    if (version !== generation) return;
    requesting.value = false;
    notice.value =
      error.response?.data?.message ||
      "Could not start the lyrics download. Try again later.";
  }
}

watch(
  () => props.trackId,
  () => {
    generation++;
    clearTimeout(timer);
    controller?.abort();
    lyrics.value = null;
    expanded.value = false;
    requesting.value = false;
    refresh();
  },
  { immediate: true },
);
onUnmounted(() => {
  generation++;
  clearTimeout(timer);
  controller?.abort();
});
</script>

<style scoped>
.lyricsPanel {
  text-align: left;
  min-width: 0;
}
header {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 24px;
}
h2 {
  margin: 0;
  font-size: 1.5rem;
}
.source {
  color: var(--text-subdued);
  font-size: 0.8rem;
}
.plainLyrics {
  white-space: pre-wrap;
  line-height: 1.9;
  font-size: 1.15rem;
}
button {
  cursor: pointer;
  border: 1px solid var(--surface-border, #444);
  border-radius: 20px;
  padding: 10px 18px;
  background: var(--bg-elevated, #242424);
  color: inherit;
  margin: 8px 8px 8px 0;
}
button:disabled {
  opacity: 0.6;
  cursor: wait;
}
button:focus-visible {
  outline: 2px solid var(--spotify-green, #1ed760);
  outline-offset: 3px;
}
.timedLyrics button,
.timedLyrics p {
  display: block;
  width: 100%;
  text-align: left;
  font: inherit;
  font-size: clamp(1.1rem, 2.3vw, 1.8rem);
  font-weight: 600;
  line-height: 1.6;
  margin: 0;
  padding: 12px 8px;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: var(--text-subdued, #aaa);
  white-space: pre-wrap;
}
.timedLyrics .active {
  color: var(--spotify-green, #1ed760);
  background: rgba(30, 215, 96, 0.08);
}
.timedLyrics button:hover {
  color: var(--text-base, white);
  background: rgba(255, 255, 255, 0.05);
}
.compact header {
  margin-bottom: 16px;
  flex-wrap: wrap;
  gap: 8px 16px;
}
.compact .timedLyrics button,
.compact .timedLyrics p,
.compact .plainLyrics {
  font-size: 1rem;
  font-weight: 400;
  line-height: 1.75;
  padding: 2px 0;
  margin: 0;
  color: var(--text-subdued);
}
.compact .timedLyrics .active {
  color: var(--text-base);
  background: transparent;
}
.compact .expanded {
  max-height: 360px;
  overflow-y: auto;
  overscroll-behavior: contain;
}
.compact .lyricsToggle {
  background: transparent;
  border: 0;
  border-radius: 4px;
  padding: 8px 0;
  margin: 8px 0 0;
  font-weight: 700;
  font-size: 0.875rem;
}
.compact .lyricsToggle:hover {
  text-decoration: underline;
}
</style>
