<template>
  <div
    class="workArtwork"
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
</template>

<script setup>
import { computed } from "vue";
import MultiSourceImage from "@/components/common/MultiSourceImage.vue";
import { formatImageUrl } from "@/utils";
const props = defineProps({ artistIds: { type: Array, default: () => [] } });
const artistIds = computed(() => [...new Set(props.artistIds)].slice(0, 4));
</script>

<style scoped>
.workArtwork {
  display: grid;
  position: relative;
  background: var(--surface-raised);
  overflow: hidden;
  border-radius: 7px;
}
.workArtwork.collage {
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
  inset: 0;
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
  font-size: 2em;
}
</style>
