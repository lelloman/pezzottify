<template>
  <router-link
    :to="`/genre/${encodeURIComponent(genre.name)}`"
    class="genreCard"
    :style="{ background: genreBackground(genre.name) }"
  >
    <MultiSourceImage
      v-if="genre.artwork_url"
      :urls="[genre.artwork_url]"
      class="genrePoster"
      alt=""
    />
    <div class="genreIdentity">
      <span class="genreName">{{ genre.name }}</span>
      <span class="trackCount">{{ formatTrackCount(genre.track_count) }}</span>
    </div>
  </router-link>
</template>
<script setup>
import MultiSourceImage from "./MultiSourceImage.vue";
import { genreBackground } from "@/utils/genreArtwork";
defineProps({ genre: { type: Object, required: true } });
const formatTrackCount = (count) =>
  `${count.toLocaleString()} ${count === 1 ? "track" : "tracks"}`;
</script>
<style scoped>
.genreCard {
  position: relative;
  isolation: isolate;
  overflow: hidden;
  aspect-ratio: 1;
  border-radius: 8px;
  text-decoration: none;
  background: #28282f;
}
.genrePoster {
  width: 100%;
  height: 100%;
  object-fit: cover;
  transition: transform 220ms ease;
}
.genreIdentity {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  justify-content: flex-end;
  padding: 20px;
  gap: 6px;
  background: linear-gradient(transparent 25%, #0005 55%, #000d);
}
.genreName {
  font-size: clamp(24px, 3cqw, 32px);
  font-weight: 800;
  line-height: 1.1;
  letter-spacing: -0.025em;
  color: #fff;
  text-transform: capitalize;
  overflow-wrap: anywhere;
}
.trackCount {
  font-size: var(--text-sm);
  color: #ffffffc9;
}
.genreCard:hover .genrePoster {
  transform: scale(1.045);
}
.genreCard:focus-visible {
  outline: 2px solid #fff;
  outline-offset: 4px;
}
@container (max-width: 560px) {
  .genreIdentity {
    padding: 14px;
  }
  .genreName {
    font-size: 22px;
  }
  .trackCount {
    font-size: 12px;
  }
}
@media (prefers-reduced-motion: reduce) {
  .genrePoster {
    transition: none;
  }
}
</style>
