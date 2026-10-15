<template>
  <img
    ref="imgRef"
    class="multiSourceImage"
    :class="{
      'image-loading': state === 'loading',
      'image-placeholder': state !== 'loaded',
    }"
    :data-image-state="state"
    :src="
      currentSrc || (state === 'error' ? unavailableImage : transparentImage)
    "
    :alt="
      state === 'error'
        ? alt
          ? alt + ' unavailable'
          : 'Image unavailable'
        : alt
    "
    :aria-busy="state === 'loading'"
  />
</template>

<script setup>
import { useDebugStore } from "@/store/debug";
import { fetchImageBlob } from "@/services/imageLoader";
import { artworkPaletteColor, PALETTE_FALLBACK } from "@/utils/androidPalette";
import { ref, watch, onMounted, onUnmounted } from "vue";
import unavailableImage from "@/assets/image-unavailable.svg";

// A valid transparent image keeps the img layout without a browser broken-image icon.
const transparentImage =
  'data:image/svg+xml,<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"/>';

const configStore = useDebugStore();
const props = defineProps({
  urls: {
    type: Array,
    required: true,
    validator: (value) => value.every((url) => typeof url === "string"),
  },
  lazy: { type: Boolean, default: true },
  palette: Boolean,
  alt: { type: String, default: "" },
});
const emit = defineEmits(["palette"]);

const imgRef = ref(null);
const currentSrc = ref("");
const state = ref("idle");
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
    if (props.palette) emit("palette", PALETTE_FALLBACK);
    let objectUrl = null;
    onCleanup(() => {
      controller.abort();
      if (objectUrl) URL.revokeObjectURL(objectUrl);
      currentSrc.value = "";
    });
    const urls = JSON.parse(urlsJson).filter(Boolean);
    state.value = !enabled ? "disabled" : !urls.length ? "error" : "idle";
    if (!enabled || !visible || !urls.length) return;
    state.value = "loading";

    void (async () => {
      for (const url of urls) {
        try {
          const blob = await fetchImageBlob(url, { signal: controller.signal });
          if (controller.signal.aborted) return;
          objectUrl = URL.createObjectURL(blob);
          const image = new Image();
          image.src = objectUrl;
          await image.decode();
          if (controller.signal.aborted) return;
          currentSrc.value = objectUrl;
          state.value = "loaded";
          if (props.palette) {
            try {
              emit("palette", artworkPaletteColor(image, url));
            } catch {
              // Canvas extraction must never prevent artwork from displaying.
              emit("palette", PALETTE_FALLBACK);
            }
          }
          return;
        } catch {
          if (controller.signal.aborted) return;
          if (objectUrl) URL.revokeObjectURL(objectUrl);
          objectUrl = null;
          // Permanent failures and exhausted retries advance to the next source.
        }
      }
      state.value = "error";
    })();
  },
  { immediate: true },
);
</script>

<style scoped>
.image-placeholder {
  background-color: var(--bg-elevated, #282828);
}

.image-loading {
  background-image: linear-gradient(
    90deg,
    transparent 25%,
    var(--bg-highlight, #3e3e3e) 50%,
    transparent 75%
  );
  background-size: 200% 100%;
  animation: image-shimmer 1.6s linear infinite;
}

@keyframes image-shimmer {
  from {
    background-position: 200% 0;
  }
  to {
    background-position: -200% 0;
  }
}

@media (prefers-reduced-motion: reduce) {
  .image-loading {
    animation: none;
  }
}
</style>
