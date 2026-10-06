<template>
  <div
    class="queueRow"
    :class="{ current }"
    @contextmenu.prevent="$emit('menu', $event)"
  >
    <button
      class="artwork"
      :disabled="!canPlay"
      :aria-label="`Play ${track?.name || 'track'}`"
      @click="$emit('play')"
    >
      <MultiSourceImage
        v-if="track?.album_id"
        :urls="chooseAlbumCoverImageUrl({ id: track.album_id })"
        alt=""
      />
      <span v-else aria-hidden="true">♫</span>
      <svg class="playIcon" viewBox="0 0 24 24" aria-hidden="true">
        <path d="M7 3v18l15-9z" />
      </svg>
    </button>
    <div class="identity">
      <RouterLink :to="`/track/${trackId}`" class="name" :title="track?.name">{{
        track?.name || (trackRef?.error ? "Track unavailable" : "Loading…")
      }}</RouterLink>
      <div class="artists">
        <LoadClickableArtistsNames
          v-if="track?.artists_ids"
          :artistsIds="track.artists_ids"
        />
      </div>
    </div>
    <span
      v-if="isAuto"
      class="autoMarker"
      title="Added by smart continuation"
      aria-label="Added by smart continuation"
      ><AiContinuationIcon
    /></span>
    <button
      class="more"
      :aria-label="`More actions for ${track?.name || 'track'}`"
      @click="$emit('menu', $event)"
    >
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <circle cx="4" cy="12" r="2" />
        <circle cx="12" cy="12" r="2" />
        <circle cx="20" cy="12" r="2" />
      </svg>
    </button>
  </div>
</template>
<script setup>
import { computed, shallowRef, watch } from "vue";
import { useUserStore } from "@/store/user";
import { useStaticsStore } from "@/store/statics";
import { chooseAlbumCoverImageUrl } from "@/utils";
import MultiSourceImage from "./MultiSourceImage.vue";
import LoadClickableArtistsNames from "./LoadClickableArtistsNames.vue";
import AiContinuationIcon from "../icons/AiContinuationIcon.vue";
const props = defineProps({
  trackId: { type: String, required: true },
  current: Boolean,
  isAuto: Boolean,
});
defineEmits(["play", "menu"]);
const statics = useStaticsStore();
const user = useUserStore();
const trackRef = shallowRef(null);
watch(
  () => props.trackId,
  (id) => {
    trackRef.value = statics.getTrack(id);
  },
  { immediate: true },
);
const track = computed(() => trackRef.value?.item);
const canPlay = computed(
  () =>
    track.value &&
    (user.isProxyModeEnabled ||
      !track.value.availability ||
      track.value.availability === "available"),
);
</script>
<style scoped>
.queueRow {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px;
  min-height: 64px;
  border-radius: 6px;
  min-width: 0;
}
.queueRow:hover,
.queueRow:focus-within {
  background: var(--surface-hover);
}
.artwork {
  position: relative;
  width: 48px;
  height: 48px;
  flex: 0 0 48px;
  border-radius: 4px;
  overflow: hidden;
  display: grid;
  place-items: center;
  background: #282828;
}
.artwork img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.artwork:disabled {
  cursor: default;
  opacity: 0.5;
}
.playIcon {
  display: none;
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  padding: 14px;
  fill: white;
  background: #0008;
}
.queueRow:hover .artwork:not(:disabled) .playIcon,
.artwork:not(:disabled):focus-visible .playIcon {
  display: block;
}
.identity {
  flex: 1;
  min-width: 0;
}
.name {
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 16px;
  line-height: 22px;
  color: var(--text-base);
  text-decoration: none;
}
.name:hover {
  text-decoration: underline;
}
.current .name {
  color: var(--spotify-green);
}
.artists {
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  font-size: 14px;
  line-height: 20px;
  color: var(--text-subdued);
}
.artists :deep(a),
.artists :deep(span) {
  color: var(--text-subdued);
  font-size: 14px;
}
.more {
  flex: 0 0 24px;
  width: 24px;
  height: 32px;
  color: var(--text-subdued);
  opacity: 0;
}
.more svg {
  width: 20px;
  height: 20px;
  fill: currentColor;
}
.queueRow:hover .more,
.queueRow:focus-within .more {
  opacity: 1;
}
.more:hover {
  color: white;
}
.autoMarker {
  width: 14px;
  height: 14px;
  flex-shrink: 0;
  color: var(--text-subdued);
}
.autoMarker :deep(svg) {
  width: 100%;
  height: 100%;
}
button:focus-visible,
a:focus-visible {
  outline: 2px solid white;
  outline-offset: 2px;
}
@media (hover: none) {
  .more {
    opacity: 1;
  }
}
</style>
