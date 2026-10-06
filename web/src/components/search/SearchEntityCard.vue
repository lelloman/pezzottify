<template>
  <article class="entityCard" :class="{ hero }">
    <RouterLink
      :to="`/${result.type.toLowerCase()}/${result.id}`"
      class="cardLink"
    >
      <MultiSourceImage
        :urls="imageUrls"
        class="artwork"
        :class="{ round: result.type === 'Artist' }"
        alt=""
      />
      <span class="name">{{ result.name }}</span>
      <span class="subtitle">{{ subtitle }}</span>
      <span
        v-if="
          availability === 'missing' ||
          availability === 'partial' ||
          availability === 'fetch_error'
        "
        class="availability"
        >{{
          availability === "partial"
            ? "Partially available"
            : availability === "fetch_error"
              ? "Download failed"
              : "Not available"
        }}</span
      >
    </RouterLink>
    <button
      type="button"
      class="playButton"
      :aria-label="`Play ${result.name}`"
      :disabled="!canPlay"
      @click="play"
    >
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <path d="M7 3v18l15-9z" />
      </svg>
    </button>
  </article>
</template>
<script setup>
import { computed } from "vue";
import MultiSourceImage from "../common/MultiSourceImage.vue";
import { formatImageUrl } from "@/utils";
import { usePlaybackStore } from "@/store/playback";
import { useUserStore } from "@/store/user";
const props = defineProps({
  result: { type: Object, required: true },
  hero: Boolean,
});
const playback = usePlaybackStore(),
  user = useUserStore();
const imageUrls = computed(() => {
  const id =
    props.result.type === "Track" ? props.result.album_id : props.result.id;
  return id ? [formatImageUrl(id)] : [];
});
const subtitle = computed(() => {
  const result = props.result;
  const artists =
    result.artist_names?.join(", ") ||
    result.artists_ids_names
      ?.map((a) => a.name || a[1])
      .filter(Boolean)
      .join(", ");
  return [
    result.type === "Track" ? "Song" : result.type,
    result.type !== "Artist" ? artists : null,
  ]
    .filter(Boolean)
    .join(" · ");
});
const availability = computed(
  () => props.result.availability || props.result.album_availability,
);
const canPlay = computed(
  () =>
    props.result.type !== "Track" ||
    user.isProxyModeEnabled ||
    !props.result.availability ||
    props.result.availability === "available",
);
const play = () => {
  if (!canPlay.value) return;
  if (props.result.type === "Album") playback.setAlbumId(props.result.id);
  else if (props.result.type === "Artist")
    playback.setArtistGreatestHits(props.result.id);
  else playback.setTrack(props.result);
};
</script>
<style scoped>
.entityCard {
  position: relative;
  min-width: 0;
  padding: 12px;
  border-radius: 6px;
}
.entityCard:hover,
.entityCard:focus-within {
  background: var(--surface-hover);
}
.cardLink {
  display: flex;
  flex-direction: column;
  gap: 8px;
  color: var(--text-base);
  text-decoration: none;
  min-width: 0;
}
.artwork {
  width: 100%;
  aspect-ratio: 1;
  object-fit: cover;
  border-radius: 4px;
  box-shadow: 0 8px 24px #0004;
  background: var(--surface-raised);
}
.artwork.round {
  border-radius: 50%;
}
.name {
  font-size: 16px;
  line-height: 22px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.availability {
  color: var(--text-subdued);
  font-size: 12px;
}
.subtitle {
  color: var(--text-subdued);
  font-size: 14px;
  line-height: 20px;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}
.hero {
  padding: 20px;
  background: var(--surface-raised);
  min-height: 248px;
}
.hero .artwork {
  width: 92px;
  height: 92px;
}
.hero .name {
  font-size: 32px;
  line-height: 38px;
  font-weight: 700;
  margin-top: 8px;
}
.hero .cardLink {
  padding-bottom: 28px;
}
.playButton {
  position: absolute;
  right: 20px;
  bottom: 76px;
  width: 48px;
  height: 48px;
  display: grid;
  place-items: center;
  background: var(--spotify-green);
  color: black;
  border-radius: 50%;
  box-shadow: 0 8px 16px #0005;
  opacity: 0;
  transform: translateY(6px);
  transition:
    opacity 150ms,
    transform 150ms;
}
.hero .playButton {
  width: 56px;
  height: 56px;
  bottom: 20px;
}
.playButton svg {
  width: 24px;
  height: 24px;
  fill: currentColor;
}
.entityCard:hover .playButton,
.entityCard:focus-within .playButton {
  opacity: 1;
  transform: none;
}
.playButton:hover:not(:disabled) {
  background: var(--spotify-green-hover);
  scale: 1.04;
}
.playButton:disabled {
  cursor: default;
}
.cardLink:focus-visible,
.playButton:focus-visible {
  outline: 2px solid white;
  outline-offset: 3px;
}
@media (hover: none) {
  .playButton {
    opacity: 1;
    transform: none;
  }
}
@media (prefers-reduced-motion: reduce) {
  .playButton {
    transition: none;
  }
}
</style>
