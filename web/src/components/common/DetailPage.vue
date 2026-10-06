<template>
  <article class="detailPage">
    <header class="detailHero">
      <MultiSourceImage
        v-if="imageUrls.length"
        :urls="imageUrls"
        :lazy="false"
        alt=""
        aria-hidden="true"
        class="heroBackdrop"
      />
      <div
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
        <h1 :class="{ longTitle: title.length > 40 }">{{ title }}</h1>
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
import MultiSourceImage from "./MultiSourceImage.vue";
defineEmits(["artwork-contextmenu"]);
defineProps({
  title: { type: String, required: true },
  kind: { type: String, required: true },
  imageUrls: { type: Array, default: () => [] },
  round: Boolean,
});
</script>
<style scoped>
.detailPage {
  min-width: 0;
  color: var(--text-base);
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
  grid-template-columns: 20px 1fr auto;
  gap: 8px;
  padding: 0 8px 12px 24px;
  border-bottom: 1px solid #ffffff15;
  color: var(--text-subdued);
  font-size: 0.8rem;
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
</style>
