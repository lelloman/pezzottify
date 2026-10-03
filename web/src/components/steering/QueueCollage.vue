<template>
  <div class="queueCollage" aria-hidden="true">
    <template v-if="albumIds.length >= 4">
      <MultiSourceImage
        v-for="albumId in albumIds.slice(0, 4)"
        :key="albumId"
        class="cell"
        :urls="[formatImageUrl(albumId)]"
      />
    </template>
    <MultiSourceImage
      v-else-if="albumIds.length"
      class="single"
      :urls="[formatImageUrl(albumIds[0])]"
    />
    <span v-else class="empty">♪</span>
  </div>
</template>

<script setup>
import { computed } from "vue";
import MultiSourceImage from "@/components/common/MultiSourceImage.vue";
import { useStaticsStore } from "@/store/statics";
import { formatImageUrl } from "@/utils";

const props = defineProps({
  tracksIds: {
    type: Array,
    default: () => [],
  },
});

const statics = useStaticsStore();

// The first few distinct albums of the queue, like a Spotify playlist cover.
const trackRefs = computed(() =>
  props.tracksIds.slice(0, 24).map((id) => statics.getTrack(id)),
);
const albumIds = computed(() => {
  const seen = [];
  for (const ref of trackRefs.value) {
    const albumId = ref?.item?.album_id;
    if (albumId && !seen.includes(albumId)) seen.push(albumId);
    if (seen.length === 4) break;
  }
  return seen;
});
</script>

<style scoped>
.queueCollage {
  position: relative;
  display: grid;
  grid-template-columns: 1fr 1fr;
  width: 100%;
  aspect-ratio: 1;
  overflow: hidden;
  border-radius: var(--radius-lg);
  background: linear-gradient(135deg, #2a2f33, #16191b);
  box-shadow: var(--shadow-lg);
}

.cell {
  width: 100%;
  aspect-ratio: 1;
  object-fit: cover;
}

.single {
  grid-column: 1 / -1;
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.cell:not([src]),
.single:not([src]) {
  opacity: 0;
}

.empty {
  grid-column: 1 / -1;
  display: grid;
  place-items: center;
  color: var(--text-subdued);
  font-size: 3rem;
}
</style>
