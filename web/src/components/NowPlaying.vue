<template>
  <section
    class="nowPlaying"
    :class="{ withoutLyrics: !hasLyrics }"
    :style="{ '--artwork-color': artworkColor }"
    aria-labelledby="nowPlayingTitle"
  >
    <header class="nowPlayingHeader">
      <span id="nowPlayingTitle">Now playing</span>
    </header>
    <p v-if="!playback.currentTrackId" class="emptyPlayback">
      Play a track to see its artwork and lyrics here.
    </p>
    <div v-else class="nowPlayingBody">
      <section class="recordPanel" aria-label="Playback controls">
        <MultiSourceImage
          :urls="imageUrls"
          :lazy="false"
          palette
          @palette="artworkColor = $event"
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
            :style="{
              '--progress': `${(playback.progressPercent || 0) * 100}%`,
            }"
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
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <path v-if="playback.isPlaying" d="M6 4h4v16H6zM14 4h4v16h-4z" />
              <path v-else d="M7 3v18l15-9z" />
            </svg>
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
          @availability="hasLyrics = $event"
        />
      </div>
    </div>
  </section>
</template>

<script setup>
import { computed, ref, watch } from "vue";
import { usePlaybackStore } from "@/store/playback";
import MultiSourceImage from "@/components/common/MultiSourceImage.vue";
import TrackLyrics from "@/components/common/TrackLyrics.vue";
import SkipPrevious from "@/components/icons/SkipPrevious.vue";
import SkipNext from "@/components/icons/SkipNext.vue";

import { PALETTE_FALLBACK } from "@/utils/androidPalette";

const playback = usePlaybackStore();
const artworkColor = ref(PALETTE_FALLBACK);
const hasLyrics = ref(false);
watch(
  () => playback.currentTrackId,
  () => {
    hasLyrics.value = false;
  },
);
const imageUrls = computed(() => {
  const track = playback.currentTrack;
  const imageId = playback.mode === "remote" ? track?.imageId : track?.albumId;
  return imageId ? [`/v1/content/image/${imageId}`] : [];
});
watch(imageUrls, () => {
  artworkColor.value = PALETTE_FALLBACK;
});
const durationMs = computed(() => playback.currentTrack?.duration || 0);
const time = (value) => {
  const seconds = Math.max(0, Math.floor(Number(value) || 0));
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
};
</script>

<style scoped>
.nowPlaying {
  display: flex;
  flex-direction: column;
  box-sizing: border-box;
  width: 100%;
  height: 100%;
  min-height: 0;
  overflow: hidden;
  color: #fff;
  background: linear-gradient(
    150deg,
    color-mix(in srgb, var(--artwork-color) 45%, #121212),
    #121212 85%
  );
}
.nowPlayingHeader {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  padding: 24px 32px 12px;
}
.nowPlayingHeader > span {
  font-size: 16px;
  font-weight: 700;
}
.emptyPlayback {
  padding: 24px 32px;
  color: #b3b3b3;
  line-height: 1.5;
}
.nowPlayingBody {
  display: grid;
  grid-template-columns: minmax(0, 0.9fr) minmax(0, 1.1fr);
  grid-template-rows: minmax(0, 1fr);
  flex: 1;
  min-height: 0;
  width: 100%;
  overflow: hidden;
}
.recordPanel {
  min-height: 0;
  min-width: 0;
  padding: 24px 32px 32px;
  overflow-y: auto;
  overscroll-behavior: contain;
}
.artwork {
  display: block;
  width: min(100%, 38vh);
  aspect-ratio: 1;
  object-fit: cover;
  border-radius: 6px;
  margin: 0 auto 24px;
  box-shadow: 0 12px 40px #0005;
}
.identity,
.seekRow,
.transport,
.remoteNotice {
  max-width: 440px;
  margin-left: auto;
  margin-right: auto;
}
.identity h1 {
  margin: 0 0 8px;
  font-size: clamp(24px, 3vw, 40px);
  line-height: 1.12;
  letter-spacing: -0.035em;
  font-weight: 800;
  overflow-wrap: anywhere;
}
.identity p {
  margin: 6px 0;
  color: #b3b3b3;
  font-size: 16px;
  line-height: 1.5;
  overflow-wrap: anywhere;
}
.identity .album {
  font-size: 13px;
}
.seekRow {
  margin-top: 24px;
}
input[type="range"] {
  --fill: #fff;
  display: block;
  appearance: none;
  width: 100%;
  height: 16px;
  margin: 0;
  background: transparent;
  cursor: pointer;
}
input[type="range"]::-webkit-slider-runnable-track {
  height: 4px;
  border-radius: 2px;
  background: linear-gradient(
    to right,
    var(--fill) var(--progress),
    #ffffff40 var(--progress)
  );
}
input[type="range"]::-moz-range-track {
  height: 4px;
  border-radius: 2px;
  background: linear-gradient(
    to right,
    var(--fill) var(--progress),
    #ffffff40 var(--progress)
  );
}
input[type="range"]::-webkit-slider-thumb {
  appearance: none;
  height: 12px;
  width: 12px;
  margin-top: -4px;
  background: white;
  border-radius: 50%;
  opacity: 0;
}
input[type="range"]::-moz-range-thumb {
  height: 12px;
  width: 12px;
  border: 0;
  background: white;
  border-radius: 50%;
  opacity: 0;
}
input[type="range"]:hover,
input[type="range"]:focus-visible {
  --fill: #1ed760;
}
input[type="range"]:hover::-webkit-slider-thumb,
input[type="range"]:focus-visible::-webkit-slider-thumb {
  opacity: 1;
}
input[type="range"]:hover::-moz-range-thumb,
input[type="range"]:focus-visible::-moz-range-thumb {
  opacity: 1;
}
input:disabled {
  cursor: default;
  opacity: 0.4;
}
.times {
  display: flex;
  justify-content: space-between;
  font-size: 12px;
  color: #b3b3b3;
  font-variant-numeric: tabular-nums;
}
.transport {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 24px;
  margin-top: 16px;
}
.transport button {
  display: grid;
  place-items: center;
  flex-shrink: 0;
  width: 40px;
  height: 40px;
  padding: 10px;
  border: 0;
  border-radius: 50%;
  background: transparent;
  color: #b3b3b3;
  cursor: pointer;
}
.transport button:hover {
  color: white;
  transform: scale(1.06);
}
.transport button:active {
  transform: scale(0.96);
}
.transport svg {
  width: 100%;
  height: 100%;
  fill: currentColor;
}
.transport .playButton {
  width: 56px;
  height: 56px;
  padding: 16px;
  background: #fff;
  color: #000;
}
.transport .playButton:hover {
  color: #000;
  background: #fff;
}
button:focus-visible,
input:focus-visible {
  outline: 2px solid white;
  outline-offset: 4px;
}
.lyricsColumn {
  min-height: 0;
  min-width: 0;
  overflow-y: auto;
  overscroll-behavior: contain;
  padding: 24px 32px 64px;
  scrollbar-gutter: stable;
}
.lyricsColumn :deep(header) {
  flex-wrap: wrap;
  gap: 8px 16px;
  margin-bottom: 24px;
}
.lyricsColumn :deep(h2) {
  font-size: 20px;
  letter-spacing: -0.02em;
}
.lyricsColumn :deep(.source) {
  font-size: 12px;
}
.lyricsColumn :deep(.timedLyrics button),
.lyricsColumn :deep(.timedLyrics p),
.lyricsColumn :deep(.plainLyrics) {
  font-size: clamp(24px, 3vw, 40px);
  font-weight: 800;
  line-height: 1.45;
  letter-spacing: -0.025em;
  color: #ffffff80;
  padding: 10px 0;
  background: transparent;
}
.lyricsColumn :deep(.timedLyrics .active),
.lyricsColumn :deep(.timedLyrics button:hover),
.lyricsColumn :deep(.plainLyrics) {
  color: white;
  background: transparent;
}
.remoteNotice {
  color: #b3b3b3;
  font-size: 12px;
  line-height: 1.5;
  margin-top: 20px;
}
@container (max-width: 700px) {
  .nowPlayingHeader {
    padding: 16px 20px 8px;
  }
  .nowPlayingBody {
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: auto minmax(0, 1fr);
  }
  .recordPanel {
    display: grid;
    grid-template-columns: 72px minmax(0, 1fr);
    align-items: center;
    gap: 12px 16px;
    padding: 12px 20px 16px;
    max-height: 40vh;
  }
  .artwork {
    width: 72px;
    border-radius: 4px;
    margin: 0;
  }
  .identity {
    margin: 0;
  }
  .identity h1 {
    font-size: 22px;
  }
  .identity p {
    font-size: 14px;
    margin: 3px 0;
  }
  .identity .album {
    font-size: 12px;
  }
  .seekRow,
  .transport,
  .remoteNotice {
    grid-column: 1 / -1;
    margin: 0;
    width: 100%;
    max-width: none;
  }
  .transport .playButton {
    width: 48px;
    height: 48px;
    padding: 13px;
  }
  .lyricsColumn {
    padding: 20px 20px 48px;
    border-top: 1px solid #ffffff12;
  }
  .lyricsColumn :deep(.timedLyrics button),
  .lyricsColumn :deep(.timedLyrics p),
  .lyricsColumn :deep(.plainLyrics) {
    font-size: 26px;
  }
}
.withoutLyrics .nowPlayingBody {
  display: flex;
  flex-direction: column;
  align-items: center;
  overflow-y: auto;
  padding: 16px 24px 32px;
  box-sizing: border-box;
}
.withoutLyrics .recordPanel {
  display: block;
  flex-shrink: 0;
  overflow: visible;
  max-height: none;
  width: min(100%, 440px);
  padding: 0;
  text-align: center;
}
.withoutLyrics .artwork {
  width: min(100%, 34vh);
  margin: 0 auto 24px;
}
.withoutLyrics .identity {
  margin: 0 auto;
}
.withoutLyrics .seekRow {
  margin-top: 24px;
}
.withoutLyrics .transport {
  margin-top: 16px;
}
.withoutLyrics .lyricsColumn {
  flex-shrink: 0;
  width: min(100%, 440px);
  box-sizing: border-box;
  padding: 24px 0 0;
  overflow: visible;
  border: 0;
  scrollbar-gutter: auto;
}
.withoutLyrics :deep(.lyricsPanel) {
  text-align: center;
  color: #b3b3b3;
  font-size: 13px;
  line-height: 1.5;
}
.withoutLyrics :deep(.lyricsPanel header) {
  display: none;
}
.withoutLyrics :deep(.lyricsPanel button) {
  background: transparent;
  font: inherit;
  font-weight: 600;
  border-color: #727272;
}
.withoutLyrics :deep(.lyricsPanel button:hover:not(:disabled)) {
  color: #fff;
  border-color: #fff;
}
@container (max-width: 700px) {
  .withoutLyrics .nowPlayingBody {
    padding: 12px 20px 24px;
  }
  .withoutLyrics .artwork {
    width: min(100%, 26vh);
    margin-bottom: 16px;
  }
  .withoutLyrics .seekRow {
    margin-top: 16px;
  }
  .withoutLyrics .lyricsColumn {
    padding-top: 16px;
  }
}
@media (prefers-reduced-motion: reduce) {
  .transport button:hover,
  .transport button:active {
    transform: none;
  }
}
</style>
