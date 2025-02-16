<template>
  <img :src="currentSrc" alt=""
    onerror="this.src='data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIxNiIgaGVpZ2h0PSIxNiIgdmlld0JveD0iMCAwIDE2IDE2Ij48L3N2Zz4=';" />
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
let loaded = false;

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
      loaded = true;
      break;
    } catch {
      // Continue to the next URL if this one fails
    }
  }
};

watch([window.globalConf.imagesEnabled, () => props.urls], ([newImagesEnabled, newUrls], [oldImagesEnabled, oldUrls]) => {
  if (!loaded && newImagesEnabled && newUrls && newUrls.length > 0) {
    currentSrc.value = '';
    loadImagesSequentially(newUrls);
  } else if (oldImagesEnabled && !newImagesEnabled) {
    loaded = false;
    currentSrc.value = '';
  }
}, { immediate: true });

</script>
