<template>
  <img :src="currentSrc" />
</template>

<script setup>
import { ref, watch } from 'vue';

const props = defineProps({
  urls: {
    type: Array,
    required: true,
    validator: (value) => value.every((url) => typeof url === 'string'),
  },
});

const currentSrc = ref('');

const tryLoadImage = (url) => {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(url);
    img.onerror = () => reject();
    img.src = url;
  });
};

const loadImagesSequentially = async (urls) => {
  for (const url of urls) {
    try {
      const successfulUrl = await tryLoadImage(url);
      currentSrc.value = successfulUrl;
      break; // Stop after the first successful load
    } catch {
      // Continue to the next URL if this one fails
    }
  }
};

watch(
  () => props.urls,
  (newUrls) => {
    if (newUrls && newUrls.length > 0) {
      currentSrc.value = ''; // Reset the image source
      loadImagesSequentially(newUrls);
    }
  },
  { immediate: true }
);
</script>
