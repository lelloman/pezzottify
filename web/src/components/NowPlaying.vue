<template>
  <Teleport to="body">
    <dialog
      ref="dialog"
      class="nowPlaying"
      aria-labelledby="nowPlayingTitle"
      @cancel.prevent="$emit('close')"
    >
      <header class="nowPlayingHeader">
        <span id="nowPlayingTitle">Now playing</span>
        <button
          type="button"
          autofocus
          aria-label="Close full-screen player"
          @click="$emit('close')"
        >
          Close <span aria-hidden="true">×</span>
        </button>
      </header>
      <div class="nowPlayingBody">
        <section class="recordPanel" aria-label="Playback controls">
          <MultiSourceImage
            :urls="imageUrls"
            :lazy="false"
            alt="Album artwork"
            class="artwork"
          />
          <div class="identity">
            <h1>{{ playback.currentTrack?.title || "Unknown track" }}</h1>
            <p>{{ playback.currentTrack?.artistName }}</p>
            <p class="album">{{ playback.currentTrack?.albumTitle }}</p>
          </div>
          <div class="seekRow">
            <input
              aria-label="Playback position"
              type="range"
              min="0"
              max="1"
              step="0.001"
              :disabled="!durationMs"
              :value="playback.progressPercent || 0"
              @change="playback.seekToPercentage(Number($event.target.value))"
            />
            <div class="times">
              <span>{{ time(playback.progressSec) }}</span
              ><span>{{ time(durationMs / 1000) }}</span>
            </div>
          </div>
          <div class="transport">
            <button
              type="button"
              aria-label="Previous track"
              @click="playback.skipPreviousTrack"
            >
              <SkipPrevious />
            </button>
            <button
              type="button"
              class="playButton"
              :aria-label="playback.isPlaying ? 'Pause' : 'Play'"
              @click="playback.playPause"
            >
              <PauseIcon v-if="playback.isPlaying" /><PlayIcon v-else />
            </button>
            <button
              type="button"
              aria-label="Next track"
              @click="playback.skipNextTrack"
            >
              <SkipNext />
            </button>
          </div>
          <p v-if="playback.mode === 'remote'" class="remoteNotice">
            Controlling playback on your connected device
          </p>
        </section>
        <div class="lyricsColumn">
          <TrackLyrics
            v-if="playback.currentTrackId"
            :trackId="playback.currentTrackId"
          />
        </div>
      </div>
    </dialog>
  </Teleport>
</template>

<script setup>
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { usePlaybackStore } from "@/store/playback";
import MultiSourceImage from "@/components/common/MultiSourceImage.vue";
import TrackLyrics from "@/components/common/TrackLyrics.vue";
import PlayIcon from "@/components/icons/PlayIcon.vue";
import PauseIcon from "@/components/icons/PauseIcon.vue";
import SkipPrevious from "@/components/icons/SkipPrevious.vue";
import SkipNext from "@/components/icons/SkipNext.vue";

defineProps({ imageUrls: { type: Array, default: () => [] } });
const emit = defineEmits(["close"]);
const playback = usePlaybackStore();
const dialog = ref(null);
const durationMs = computed(() => playback.currentTrack?.duration || 0);
const time = (value) => {
  const seconds = Math.max(0, Math.floor(Number(value) || 0));
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
};
let previousOverflow;
const opener = document.activeElement;
onMounted(() => {
  previousOverflow = document.body.style.overflow;
  document.body.style.overflow = "hidden";
  dialog.value.showModal();
});
onBeforeUnmount(() => {
  dialog.value?.close();
  document.body.style.overflow = previousOverflow;
  if (opener?.isConnected) opener.focus();
});
watch(
  () => playback.currentTrackId,
  (id) => {
    if (!id) emit("close");
  },
);
</script>

<style scoped>
.nowPlaying {
  font: inherit;
  position: fixed;
  inset: 0;
  box-sizing: border-box;
  width: 100vw;
  height: 100dvh;
  max-width: none;
  max-height: none;
  margin: 0;
  padding: 0;
  border: 0;
  color: var(--text-base, #f5f5f5);
  background: radial-gradient(
    ellipse at top left,
    #19352d 0,
    #111b19 38%,
    #0b0d0e 75%
  );
}
.nowPlaying[open] {
  display: flex;
  flex-direction: column;
}
.nowPlayingHeader {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 20px clamp(20px, 4vw, 64px);
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
}
.nowPlayingHeader > span {
  text-transform: uppercase;
  letter-spacing: 0.16em;
  font-size: 0.8rem;
}
button {
  cursor: pointer;
  background: transparent;
  border: 0;
  color: inherit;
  font: inherit;
  border-radius: 24px;
  padding: 10px 16px;
}
button:hover {
  background: rgba(255, 255, 255, 0.1);
}
button:focus-visible,
input:focus-visible {
  outline: 2px solid var(--spotify-green, #1ed760);
  outline-offset: 4px;
}
.nowPlayingHeader button span {
  padding-left: 14px;
  font-size: 1.5rem;
}
.nowPlayingBody {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  flex: 1;
  min-height: 0;
  width: 100%;
  max-width: 1500px;
  margin: 0 auto;
}
.recordPanel {
  padding: 32px clamp(24px, 5vw, 80px);
  overflow-y: auto;
  text-align: center;
}
.artwork {
  display: block;
  width: min(100%, 38vh);
  aspect-ratio: 1;
  object-fit: cover;
  border-radius: 16px;
  margin: 0 auto 24px;
  box-shadow: 0 20px 70px #0008;
}
.identity h1 {
  font-size: clamp(1.4rem, 3vw, 2.3rem);
  margin: 0 0 8px;
  overflow-wrap: anywhere;
}
.identity p {
  margin: 6px 0;
  color: var(--text-subdued, #aaa);
}
.identity .album {
  font-size: 0.85rem;
}
.seekRow {
  margin-top: 28px;
}
input {
  width: 100%;
  accent-color: var(--spotify-green, #1ed760);
  cursor: pointer;
}
.times {
  display: flex;
  justify-content: space-between;
  font-size: 0.75rem;
  color: var(--text-subdued, #aaa);
  font-variant-numeric: tabular-nums;
}
.transport {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 22px;
  margin-top: 16px;
}
.transport button {
  width: 48px;
  height: 48px;
  padding: 12px;
}
.transport svg {
  width: 100%;
  height: 100%;
  fill: currentColor;
}
.transport .playButton {
  width: 64px;
  height: 64px;
  padding: 18px;
  background: var(--spotify-green, #1ed760);
  color: #071108;
  border-radius: 50%;
}
.lyricsColumn {
  overflow-y: auto;
  padding: 36px clamp(24px, 4vw, 64px) 80px;
  border-left: 1px solid rgba(255, 255, 255, 0.08);
}
.remoteNotice {
  color: var(--text-subdued, #aaa);
  font-size: 0.8rem;
  margin-top: 24px;
}
@media (max-width: 767px) {
  .nowPlayingBody {
    display: block;
    overflow-y: auto;
  }
  .recordPanel,
  .lyricsColumn {
    overflow: visible;
  }
  .artwork {
    width: min(65vw, 30vh);
  }
  .recordPanel {
    padding: 24px;
  }
  .lyricsColumn {
    border-left: 0;
    border-top: 1px solid rgba(255, 255, 255, 0.08);
    padding: 28px 24px 60px;
  }
  .nowPlayingHeader {
    padding: 12px 20px;
  }
}
</style>
