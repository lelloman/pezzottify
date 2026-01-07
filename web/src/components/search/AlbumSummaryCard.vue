<template>
  <div class="albumCard" @click="handleClick">
    <MultiSourceImage :urls="[imageUrl]" alt="Album" class="albumImage" />
    <div class="albumInfo">
      <span class="albumName">{{ album.name }}</span>
      <span class="albumMeta">{{ albumMeta }}</span>
    </div>
  </div>
</template>

<script setup>
import { computed } from "vue";
import { useRouter } from "vue-router";
import { computedImageUrl } from "@/utils";
import MultiSourceImage from "@/components/common/MultiSourceImage.vue";

const props = defineProps({
  album: {
    type: Object,
    required: true,
  },
});

const router = useRouter();

// Image endpoint takes the album ID directly
const imageUrl = computedImageUrl(props.album.id);

const albumMeta = computed(() => {
  const parts = [];
  if (props.album.release_year) {
    parts.push(props.album.release_year);
  }
  if (props.album.track_count) {
    parts.push(`${props.album.track_count} tracks`);
  }
  return parts.join(" - ");
});

const handleClick = () => {
  router.push("/album/" + props.album.id);
};
</script>

<style scoped>
.albumCard {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px;
  border-radius: var(--radius-md);
  cursor: pointer;
  transition: background-color 0.2s ease;
}

.albumCard:hover {
  background-color: var(--bg-elevated-highlight);
}

.albumImage {
  width: 100%;
  aspect-ratio: 1;
  border-radius: var(--radius-sm);
  object-fit: cover;
}

.albumInfo {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.albumName {
  font-weight: var(--font-medium);
  color: var(--text-base);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.albumMeta {
  font-size: var(--text-sm);
  color: var(--text-subdued);
}
</style>
