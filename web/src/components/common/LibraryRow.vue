<template>
  <li class="libraryRow" :class="{ selected, playing, collapsed }">
    <RouterLink
      :to="`/${entry.type}/${entry.id}`"
      class="libraryLink"
      :aria-current="selected ? 'page' : undefined"
      :aria-label="`${entry.name}, ${entry.subtitle}`"
      :title="`${entry.name} — ${entry.subtitle}`"
    >
      <div
        class="artwork"
        :class="{
          round: entry.type === 'artist',
          collage: entry.images.length > 1,
        }"
      >
        <MultiSourceImage
          v-for="(urls, index) in entry.images"
          :key="index"
          :urls="urls"
        />
        <svg v-if="!entry.images.length" viewBox="0 0 24 24" aria-hidden="true">
          <path
            d="M9 4v11.2a3.5 3.5 0 1 0 2 3.2V8l8-2v7.2a3.5 3.5 0 1 0 2 3.2V1z"
          />
        </svg>
      </div>
      <div class="identity">
        <span class="name">{{ entry.name }}</span
        ><span class="subtitle">{{ entry.subtitle }}</span>
      </div>
      <svg
        v-if="playing"
        class="playingIcon"
        viewBox="0 0 24 24"
        aria-label="Playing"
      >
        <path
          d="M3 9h4l5-4v14l-5-4H3zM15 8a6 6 0 0 1 0 8M18 5a10 10 0 0 1 0 14"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
        />
      </svg>
    </RouterLink>
    <button
      v-if="entry.ready && !collapsed"
      type="button"
      class="playOverlay"
      :aria-label="`Play ${entry.name}`"
      :title="`Play ${entry.name}`"
      :disabled="busy"
      @click="$emit('play', entry)"
    >
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <path d="M7 3v18l15-9z" />
      </svg>
    </button>
  </li>
</template>
<script setup>
import MultiSourceImage from "./MultiSourceImage.vue";
defineProps({
  entry: { type: Object, required: true },
  selected: Boolean,
  playing: Boolean,
  collapsed: Boolean,
  busy: Boolean,
});
defineEmits(["play"]);
</script>
<style scoped>
.libraryRow {
  position: relative;
  min-width: 0;
  border-radius: 6px;
  list-style: none;
}
.libraryLink {
  display: flex;
  align-items: center;
  gap: 12px;
  height: 64px;
  padding: 8px;
  border-radius: 6px;
  color: var(--text-base);
  text-decoration: none;
}
.libraryRow:hover {
  background: #1f1f1f;
}
.libraryRow.selected {
  background: #ffffff1a;
}
.libraryRow.selected:hover {
  background: #ffffff24;
}
.artwork {
  display: grid;
  place-items: center;
  flex: 0 0 48px;
  width: 48px;
  height: 48px;
  overflow: hidden;
  background: #282828;
  border-radius: 4px;
  color: var(--text-subdued);
}
.artwork.round {
  border-radius: 50%;
}
.artwork img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  min-height: 0;
}
.artwork.collage {
  grid-template-columns: repeat(2, 1fr);
  grid-template-rows: repeat(2, 1fr);
}
.artwork > svg {
  width: 24px;
  height: 24px;
  fill: currentColor;
}
.identity {
  display: flex;
  flex: 1;
  min-width: 0;
  flex-direction: column;
  gap: 2px;
}
.name {
  font-size: 16px;
  line-height: 22px;
  font-weight: 400;
}
.subtitle {
  font-size: 14px;
  line-height: 20px;
  color: var(--text-subdued);
}
.name,
.subtitle {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.playing .name,
.playingIcon {
  color: var(--spotify-green);
}
.playingIcon {
  width: 16px;
  height: 16px;
  flex-shrink: 0;
}
.playOverlay {
  position: absolute;
  left: 8px;
  top: 8px;
  width: 48px;
  height: 48px;
  display: grid;
  place-items: center;
  border-radius: 4px;
  background: #0008;
  color: white;
  opacity: 0;
  transition: opacity var(--transition-fast);
}
.playOverlay svg {
  width: 24px;
  height: 24px;
  fill: currentColor;
}
.libraryRow:hover .playOverlay,
.playOverlay:focus-visible {
  opacity: 1;
}
.playOverlay:disabled {
  cursor: wait;
}
.libraryLink:focus-visible,
.playOverlay:focus-visible {
  outline: 2px solid white;
  outline-offset: -2px;
}
.collapsed .identity,
.collapsed .playingIcon {
  display: none;
}
.collapsed.playing .artwork {
  outline: 2px solid var(--spotify-green);
  outline-offset: -2px;
}
</style>
