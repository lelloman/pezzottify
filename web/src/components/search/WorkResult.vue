<template>
  <RouterLink
    class="searchResultRow workResult"
    :to="{ name: 'work', params: { workId: result.id } }"
  >
    <div
      class="searchResultImage creatorArtwork"
      :class="{ collage: artistIds.length > 1 }"
      aria-hidden="true"
    >
      <template v-if="artistIds.length">
        <div v-for="id in artistIds" :key="id" class="portrait">
          <span class="imageFallback">♪</span>
          <MultiSourceImage :urls="[formatImageUrl(id)]" />
        </div>
      </template>
      <span v-else class="imageFallback">♪</span>
    </div>
    <div class="column">
      <span class="title" :title="result.title">{{ result.title }}</span>
      <span class="creators" :title="creators">{{
        creators || "Unknown creator"
      }}</span>
    </div>
    <span
      v-if="result.composition_year"
      class="year"
      :title="`Composed ${result.composition_year}`"
      :aria-label="`Composed ${result.composition_year}`"
      >{{ result.composition_year }}</span
    >
  </RouterLink>
</template>

<script setup>
import "@/assets/search.css";
import { computed } from "vue";
import MultiSourceImage from "@/components/common/MultiSourceImage.vue";
import { formatImageUrl } from "@/utils";

const props = defineProps({ result: { type: Object, required: true } });
const artistIds = computed(() =>
  [...new Set(props.result.creator_artist_ids || [])].slice(0, 4),
);
const creators = computed(() => (props.result.creators || []).join(", "));
</script>

<style scoped>
.workResult {
  text-decoration: none;
  box-sizing: border-box;
  min-width: 0;
}
.workResult:focus-visible {
  outline: 2px solid var(--accent-color);
  outline-offset: -2px;
}
.creatorArtwork {
  display: grid;
  position: relative;
  background: var(--surface-raised);
}
.creatorArtwork.collage {
  grid-template-columns: repeat(2, minmax(0, 1fr));
}
.portrait {
  position: relative;
  min-height: 0;
  overflow: hidden;
}
.portrait:last-child:nth-child(3) {
  grid-column: 1 / -1;
}
.portrait img {
  position: absolute;
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.portrait img:not([src]) {
  visibility: hidden;
}
.imageFallback {
  display: grid;
  place-items: center;
  position: absolute;
  inset: 0;
  color: var(--text-subdued);
  font-size: 24px;
}
.column {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-width: 0;
  margin-right: 8px;
}
.title {
  font-size: 16px;
  font-weight: bold;
}
.creators {
  font-size: 14px;
  color: var(--text-subdued);
}
.title,
.creators {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.year {
  flex-shrink: 0;
  font-size: 14px;
  font-variant-numeric: tabular-nums;
}
</style>
