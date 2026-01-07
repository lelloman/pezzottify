<template>
  <img
    ref="imgRef"
    :src="currentSrc"
    alt=""
    loading="lazy"
    onerror="
      this.src =
        'data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIxNiIgaGVpZ2h0PSIxNiIgdmlld0JveD0iMCAwIDE2IDE2Ij48L3N2Zz4='
    "
  />
</template>

<script setup>
import { useDebugStore } from "@/store/debug";
import { ref, watch, onMounted, onUnmounted } from "vue";

const configStore = useDebugStore();

const props = defineProps({
  urls: {
    type: Array,
    required: true,
    validator: (value) => value.every((url) => typeof url === "string"),
  },
  lazy: {
    type: Boolean,
    default: true,
  },
});

const imgRef = ref(null);
const currentSrc = ref("");
const isVisible = ref(false);
let loaded = false;
let observer = null;

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

const tryLoad = () => {
  if (
    !loaded &&
    configStore.imagesEnabled &&
    props.urls &&
    props.urls.length > 0
  ) {
    currentSrc.value = "";
    loadImagesSequentially(props.urls);
  }
};

onMounted(() => {
  if (!props.lazy) {
    isVisible.value = true;
    return;
  }

  observer = new IntersectionObserver(
    (entries) => {
      const entry = entries[0];
      if (entry.isIntersecting) {
        isVisible.value = true;
        tryLoad();
        observer?.disconnect();
        observer = null;
      }
    },
    {
      rootMargin: "100px", // Start loading 100px before entering viewport
    },
  );

  if (imgRef.value) {
    observer.observe(imgRef.value);
  }
});

onUnmounted(() => {
  observer?.disconnect();
  observer = null;
});

watch(
  [() => configStore.imagesEnabled, () => props.urls, isVisible],
  ([newImagesEnabled, newUrls, nowVisible], [oldImagesEnabled, oldUrls]) => {
    if (newUrls != oldUrls) {
      loaded = false;
    }

    // Only load if visible (or lazy is disabled)
    if (!nowVisible && props.lazy) {
      return;
    }

    if (!loaded && newImagesEnabled && newUrls && newUrls.length > 0) {
      currentSrc.value = "";
      loadImagesSequentially(newUrls);
    } else if (oldImagesEnabled && !newImagesEnabled) {
      loaded = false;
      currentSrc.value = "";
    }
  },
  { immediate: true },
);
</script>
