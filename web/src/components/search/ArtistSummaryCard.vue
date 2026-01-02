<template>
  <div class="artistCard" @click="handleClick">
    <MultiSourceImage :urls="[imageUrl]" alt="Artist" class="artistImage" />
    <span class="artistName">{{ artist.name }}</span>
  </div>
</template>

<script setup>
import { useRouter } from "vue-router";
import { computedImageUrl } from "@/utils";
import MultiSourceImage from "@/components/common/MultiSourceImage.vue";

const props = defineProps({
  artist: {
    type: Object,
    required: true,
  },
});

const router = useRouter();

const imageUrl = computedImageUrl(props.artist.image_id);

const handleClick = () => {
  router.push("/artist/" + props.artist.id);
};
</script>

<style scoped>
.artistCard {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
  padding: 12px;
  border-radius: var(--radius-md);
  cursor: pointer;
  transition: background-color 0.2s ease;
}

.artistCard:hover {
  background-color: var(--bg-elevated-highlight);
}

.artistImage {
  width: 80px;
  height: 80px;
  border-radius: 50%;
  object-fit: cover;
}

.artistName {
  font-weight: var(--font-medium);
  color: var(--text-base);
  text-align: center;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 100%;
}
</style>
