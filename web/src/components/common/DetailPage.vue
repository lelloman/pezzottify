<template>
  <article
    ref="pageElement"
    class="detailPage"
    :class="{ tinted, paletteDetail: !banner }"
    :style="{
      '--artwork-color': artworkColor,
      '--banner-image-opacity': 1 - bannerProgress,
      '--banner-title-opacity': titleProgress,
    }"
  >
    <MultiSourceImage
      v-if="!banner && imageUrls.length"
      :urls="imageUrls"
      :lazy="false"
      palette
      class="paletteSource"
      alt=""
      aria-hidden="true"
      @palette="artworkColor = $event"
    />
    <div v-if="banner" class="stickyArtistHeader" aria-hidden="true">
      <div class="stickyArtistTitle">{{ title }}</div>
    </div>
    <header
      ref="heroElement"
      class="detailHero"
      :class="{ artistBanner: banner }"
      @contextmenu="banner && $emit('artwork-contextmenu', $event)"
    >
      <MultiSourceImage
        v-if="imageUrls.length && banner"
        :urls="imageUrls"
        :lazy="false"
        alt=""
        aria-hidden="true"
        class="heroBackdrop"
      />
      <div
        v-if="!banner"
        class="detailArtwork"
        :class="{ round }"
        @contextmenu="$emit('artwork-contextmenu', $event)"
      >
        <slot name="artwork">
          <MultiSourceImage
            v-if="imageUrls.length"
            :urls="imageUrls"
            :lazy="false"
            :alt="`${title} artwork`"
          />
          <svg v-else viewBox="0 0 100 100" aria-hidden="true">
            <path
              d="M40 68V28l36-8v40M40 40l36-8"
              fill="none"
              stroke="currentColor"
              stroke-width="6"
            />
            <ellipse cx="29" cy="70" rx="14" ry="10" fill="currentColor" />
            <ellipse cx="65" cy="62" rx="14" ry="10" fill="currentColor" />
          </svg>
        </slot>
      </div>
      <div class="detailIdentity">
        <p class="detailKind">{{ kind }}</p>
        <h1 ref="titleElement" :class="{ longTitle: title.length > 40 }">
          {{ title }}
        </h1>
        <div class="detailMeta"><slot name="meta" /></div>
      </div>
    </header>
    <div class="detailBody">
      <div v-if="$slots.actions" class="detailActionBar">
        <slot name="actions" />
      </div>
      <slot />
    </div>
  </article>
</template>
<script setup>
import {
  ref,
  onMounted,
  onBeforeUnmount,
  onActivated,
  onDeactivated,
  nextTick,
  watch,
} from "vue";
import MultiSourceImage from "./MultiSourceImage.vue";
import { PALETTE_FALLBACK } from "@/utils/androidPalette";
defineEmits(["artwork-contextmenu"]);
const props = defineProps({
  title: { type: String, required: true },
  kind: { type: String, required: true },
  imageUrls: { type: Array, default: () => [] },
  round: Boolean,
  banner: Boolean,
  tinted: Boolean,
});
const artworkColor = ref(PALETTE_FALLBACK);
watch(
  () => JSON.stringify(props.imageUrls),
  () => {
    artworkColor.value = PALETTE_FALLBACK;
  },
  { flush: "sync" },
);
const pageElement = ref(null),
  heroElement = ref(null),
  titleElement = ref(null);
const bannerProgress = ref(0),
  titleProgress = ref(0);
let scrollRoot = null,
  resizeObserver = null,
  frame = 0;
const clamp = (value) => Math.max(0, Math.min(1, value));
const updateBanner = () => {
  frame = 0;
  if (!scrollRoot || !heroElement.value) return;
  const top = scrollRoot.getBoundingClientRect().top + scrollRoot.clientTop;
  const hero = heroElement.value.getBoundingClientRect();
  const title = titleElement.value.getBoundingClientRect();
  bannerProgress.value = clamp(
    (top - hero.top) / Math.max(1, hero.height - 56),
  );
  titleProgress.value = clamp((top + 56 - title.bottom) / 32);
};
const scheduleUpdate = () => {
  if (!frame) frame = requestAnimationFrame(updateBanner);
};
const stopTracking = () => {
  scrollRoot?.removeEventListener("scroll", scheduleUpdate);
  resizeObserver?.disconnect();
  resizeObserver = null;
  scrollRoot = null;
  cancelAnimationFrame(frame);
  frame = 0;
};
const startTracking = async () => {
  if (!props.banner) return;
  await nextTick();
  stopTracking();
  scrollRoot = pageElement.value?.closest(".mainContent");
  if (!scrollRoot) return;
  scrollRoot.addEventListener("scroll", scheduleUpdate, { passive: true });
  resizeObserver = new ResizeObserver(scheduleUpdate);
  resizeObserver.observe(scrollRoot);
  resizeObserver.observe(heroElement.value);
  resizeObserver.observe(titleElement.value);
  updateBanner();
};
onMounted(startTracking);
onActivated(startTracking);
onDeactivated(stopTracking);
onBeforeUnmount(stopTracking);
</script>
<style scoped>
.detailPage {
  min-width: 0;
  color: var(--text-base);
}
.tinted {
  position: relative;
  isolation: isolate;
  background: #121212;
}
.paletteSource {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  pointer-events: none;
  opacity: 0;
}
.paletteDetail {
  background: #121212;
}
.paletteDetail .detailHero {
  background: linear-gradient(
    color-mix(in srgb, var(--artwork-color) 65%, #121212),
    color-mix(in srgb, var(--artwork-color) 40%, #121212)
  );
}
.paletteDetail .detailBody {
  background: linear-gradient(
    color-mix(in srgb, var(--artwork-color) 22%, #121212),
    #121212 280px
  );
}
.detailHero {
  position: relative;
  isolation: isolate;
  overflow: hidden;
  display: grid;
  grid-template-columns: clamp(150px, 24cqw, 232px) minmax(0, 1fr);
  align-items: end;
  gap: 24px;
  padding: 40px var(--detail-gutter) 24px;
  background: linear-gradient(140deg, #343035, #202020 65%, #181818);
}
.heroBackdrop {
  position: absolute;
  z-index: -1;
  inset: -20%;
  width: 140%;
  height: 140%;
  object-fit: cover;
  filter: blur(65px);
  opacity: 0.35;
  pointer-events: none;
}
.detailArtwork {
  aspect-ratio: 1;
  overflow: hidden;
  background: linear-gradient(135deg, #51426c, #26242c);
  border-radius: 4px;
  box-shadow: 0 8px 36px #0005;
  display: grid;
  place-items: center;
  color: #ffffffa0;
}
.detailArtwork.round {
  border-radius: 50%;
}
.detailArtwork :deep(img),
.detailArtwork :deep(.workArtwork) {
  width: 100%;
  height: 100%;
  object-fit: cover;
}
.detailArtwork > svg {
  width: 60%;
  height: 60%;
}
.detailIdentity {
  min-width: 0;
}
.detailKind {
  font-size: var(--text-sm);
  font-weight: 400;
  margin: 0 0 12px;
}
.detailIdentity h1 {
  margin: 0 0 20px;
  font-size: clamp(2rem, 7cqw, 6rem);
  font-weight: 800;
  line-height: 1.04;
  letter-spacing: -0.02em;
  overflow-wrap: anywhere;
}
.detailIdentity h1.longTitle {
  font-size: clamp(2rem, 4cqw, 3rem);
}
.detailMeta {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px 14px;
  color: var(--text-subdued);
  font-size: 0.875rem;
  line-height: 1.5;
}
.detailMeta :deep(p) {
  margin: 0;
}
.detailMeta :deep(a),
.detailMeta :deep(button) {
  color: var(--text-base);
}
.detailBody {
  padding: 0 var(--detail-gutter) 40px;
  background: linear-gradient(#ffffff03, transparent 180px);
}
.detailActionBar {
  padding: 24px 0;
}
.detailBody :deep(.detailSupporting) {
  margin-top: 36px;
  max-width: 900px;
}
.detailBody :deep(.detailSectionTitle) {
  margin: 0 0 16px;
  font-size: var(--text-2xl);
  font-weight: 700;
  letter-spacing: -0.02em;
}
.detailBody :deep(.detailSupporting p) {
  color: var(--text-subdued);
  line-height: 1.7;
}
.detailBody :deep(.detailTrackHeading) {
  display: grid;
  grid-template-columns: 16px minmax(0, 1fr) auto;
  gap: 16px;
  padding: 0 16px 12px;
  border-bottom: 1px solid #ffffff15;
  color: var(--text-subdued);
  font-size: var(--text-sm);
}
.detailBody :deep(.detailTrackHeading span:first-child) {
  text-align: center;
}
@container (max-width:560px) {
  .detailHero {
    grid-template-columns: minmax(0, 1fr);
    gap: 24px;
    padding: 24px 20px;
  }
  .detailArtwork {
    width: min(60cqw, 220px);
    justify-self: center;
  }
  .detailIdentity h1 {
    font-size: clamp(2rem, 8cqw, 3rem);
    margin-bottom: 14px;
  }
  .detailBody {
    padding: 0 16px 32px;
  }
}
.stickyArtistHeader {
  position: sticky;
  top: 0;
  height: 0;
  z-index: 20;
  pointer-events: none;
}
.stickyArtistTitle {
  display: block;
  line-height: 56px;
  min-width: 0;
  height: 56px;
  padding: 0 var(--detail-gutter);
  background: #202020;
  color: var(--text-base);
  font-size: 24px;
  font-weight: 700;
  opacity: var(--banner-title-opacity, 0);
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}
@container (max-width:560px) {
  .stickyArtistTitle {
    padding-inline: 20px;
    font-size: 20px;
  }
}
/* Artist details use the portrait as a cover crop, never a stretched image. */
.artistBanner {
  display: flex;
  align-items: flex-end;
  min-height: clamp(280px, 38cqw, 420px);
  padding: 40px var(--detail-gutter) 32px;
  background: linear-gradient(135deg, #514b5b, #27252c 60%, #181818);
}
.artistBanner .heroBackdrop {
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: cover;
  object-position: center 35%;
  filter: none;
  opacity: var(--banner-image-opacity, 1);
}
.artistBanner .heroBackdrop:not([src]) {
  visibility: hidden;
}
.artistBanner::after {
  content: "";
  position: absolute;
  z-index: -1;
  inset: 0;
  background: linear-gradient(
      180deg,
      #0000000d 0%,
      #00000026 30%,
      #000000b3 100%
    ),
    linear-gradient(90deg, #0004, transparent 75%);
  pointer-events: none;
}
.artistBanner .detailIdentity {
  width: 100%;
  text-shadow: 0 2px 16px #0005;
}
.artistBanner .detailIdentity h1 {
  font-size: clamp(48px, 9cqw, 96px);
  margin-bottom: 16px;
}
.artistBanner .detailIdentity h1.longTitle {
  font-size: clamp(32px, 6cqw, 64px);
}
.artistBanner .detailMeta {
  color: #ffffffe6;
}
@container (max-width:560px) {
  .artistBanner {
    min-height: 280px;
    padding: 24px 20px;
  }
  .artistBanner .detailIdentity h1 {
    font-size: clamp(40px, 10cqw, 64px);
  }
}
</style>
