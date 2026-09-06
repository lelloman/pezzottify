<template>
  <img ref="imgRef" :src="currentSrc || undefined" alt="" />
</template>

<script setup>
import { useDebugStore } from "@/store/debug";
import { fetchImageBlob } from "@/services/imageLoader";
import { ref, watch, onMounted, onUnmounted } from "vue";

const configStore = useDebugStore();
const props = defineProps({
  urls: {
    type: Array,
    required: true,
    validator: (value) => value.every((url) => typeof url === "string"),
  },
  lazy: { type: Boolean, default: true },
});

const imgRef = ref(null);
const currentSrc = ref("");
const isVisible = ref(false);
let observer = null;

onMounted(() => {
  observer = new IntersectionObserver(
    (entries) => {
      if (entries[0].isIntersecting) {
        isVisible.value = true;
        observer?.disconnect();
        observer = null;
      }
    },
    { rootMargin: "100px" },
  );
  observer.observe(imgRef.value);
});

onUnmounted(() => observer?.disconnect());

watch(
  // Compare URL contents so parent renders with fresh arrays don't restart requests.
  [
    () => JSON.stringify(props.urls || []),
    () => configStore.imagesEnabled,
    () => !props.lazy || isVisible.value,
  ],
  ([urlsJson, enabled, visible], _, onCleanup) => {
    const controller = new AbortController();
    let objectUrl = null;
    onCleanup(() => {
      controller.abort();
      if (objectUrl) URL.revokeObjectURL(objectUrl);
      currentSrc.value = "";
    });
    if (!enabled || !visible) return;

    void (async () => {
      for (const url of JSON.parse(urlsJson)) {
        if (!url) continue;
        try {
          const blob = await fetchImageBlob(url, { signal: controller.signal });
          if (controller.signal.aborted) return;
          objectUrl = URL.createObjectURL(blob);
          const image = new Image();
          image.src = objectUrl;
          await image.decode();
          if (controller.signal.aborted) return;
          currentSrc.value = objectUrl;
          return;
        } catch {
          if (controller.signal.aborted) return;
          if (objectUrl) URL.revokeObjectURL(objectUrl);
          objectUrl = null;
          // Permanent failures and exhausted retries advance to the next source.
        }
      }
    })();
  },
  { immediate: true },
);
</script>
