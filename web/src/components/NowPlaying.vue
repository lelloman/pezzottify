<template>
  <section class="nowPlaying" aria-labelledby="nowPlayingTitle">
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
        />
      </div>
    </div>
  </section>
</template>

<script setup>
import { computed } from "vue";
import { usePlaybackStore } from "@/store/playback";
import MultiSourceImage from "@/components/common/MultiSourceImage.vue";
import TrackLyrics from "@/components/common/TrackLyrics.vue";
import SkipPrevious from "@/components/icons/SkipPrevious.vue";
import SkipNext from "@/components/icons/SkipNext.vue";

const playback = usePlaybackStore();
const imageUrls = computed(() => {
  const track = playback.currentTrack;
  const imageId = playback.mode === "remote" ? track?.imageId : track?.albumId;
  return imageId ? [`/v1/content/image/${imageId}`] : [];
});
const durationMs = computed(() => playback.currentTrack?.duration || 0);
const time = (value) => {
  const seconds = Math.max(0, Math.floor(Number(value) || 0));
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
};
</script>

<style scoped>
.nowPlaying {
  font: inherit;
  display: flex;
  flex-direction: column;
  box-sizing: border-box;
  width: 100%;
  height: 100%;
  min-height: 0;
  overflow: hidden;
  margin: 0;
  padding: 0;
  color: var(--text-base, #f5f5f5);
  background: radial-gradient(
    ellipse at top left,
    #19352d 0,
    #111b19 38%,
    #0b0d0e 75%
  );
}
.nowPlayingHeader {
  flex-shrink: 0;
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
.emptyPlayback {
  padding: 24px;
  color: var(--text-subdued, #aaa);
}
.nowPlayingBody {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  grid-template-rows: minmax(0, 1fr);
  overflow: hidden;
  flex: 1;
  min-height: 0;
  width: 100%;
  max-width: 1500px;
  margin: 0 auto;
}
.recordPanel {
  min-height: 0;
  min-width: 0;
  overscroll-behavior: contain;
  padding: 24px;
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
  display: grid;
  place-items: center;
  flex-shrink: 0;
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
  padding: 16px;
  background: var(--spotify-green, #1ed760);
  color: #fff;
  border-radius: 50%;
}
.lyricsColumn {
  min-height: 0;
  min-width: 0;
  overscroll-behavior: contain;
  overflow-y: auto;
  padding: 24px;
  border-left: 1px solid rgba(255, 255, 255, 0.08);
}
.remoteNotice {
  color: var(--text-subdued, #aaa);
  font-size: 0.8rem;
  margin-top: 24px;
}
@container (max-width: 700px) {
  .nowPlayingBody {
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: minmax(0, 1fr) minmax(0, 1fr);
  }
  .recordPanel {
    display: grid;
    grid-template-columns: 80px minmax(0, 1fr);
    align-content: start;
    align-items: center;
    gap: 12px;
    padding: 16px;
  }
  .artwork {
    width: 80px;
    margin: 0;
  }
  .identity {
    text-align: left;
  }
  .identity h1 {
    font-size: 1.2rem;
  }
  .seekRow,
  .transport,
  .remoteNotice {
    grid-column: 1 / -1;
    margin-top: 0;
  }
  .lyricsColumn {
    border-left: 0;
    border-top: 1px solid rgba(255, 255, 255, 0.08);
    padding: 20px;
  }
  .nowPlayingHeader {
    padding: 12px 20px;
  }
}
</style>
