<template>
  <div
    class="steeringArtwork"
    :class="[`size-${size}`, shapeClass, { concept: isConcept }]"
    :style="isConcept ? { backgroundColor: color } : null"
    aria-hidden="true"
  >
    <template v-if="isConcept">
      <span v-if="showLabel" class="conceptLabel">{{ label }}</span>
      <span class="conceptShape" />
    </template>
    <template v-else>
      <span class="fallback">{{ initial }}</span>
      <MultiSourceImage v-if="urls.length" class="image" :urls="urls" />
    </template>
  </div>
</template>

<script setup>
import { computed } from "vue";
import MultiSourceImage from "@/components/common/MultiSourceImage.vue";
import { useStaticsStore } from "@/store/statics";
import { formatImageUrl } from "@/utils";
import { tileColor } from "@/utils/steeringArt";

const props = defineProps({
  reference: {
    type: Object,
    required: true,
  },
  // xs 32px, sm 48px, md 72px, lg fills its container (square).
  size: {
    type: String,
    default: "sm",
  },
  showLabel: {
    type: Boolean,
    default: false,
  },
});

const statics = useStaticsStore();

const isConcept = computed(() => props.reference.entity_type === "concept");
const label = computed(
  () => props.reference.label || props.reference.entity_id || "",
);
const initial = computed(() => label.value.trim().charAt(0).toUpperCase());
const color = computed(() => tileColor(props.reference.entity_id));
const shapeClass = computed(() =>
  props.reference.entity_type === "artist" ? "round" : "square",
);

// Tracks show their album cover; the track is fetched on demand.
const trackRef = computed(() =>
  props.reference.entity_type === "track"
    ? statics.getTrack(props.reference.entity_id)
    : null,
);
const urls = computed(() => {
  const { entity_type: type, entity_id: id } = props.reference;
  if (type === "album" || type === "artist") return [formatImageUrl(id)];
  if (type === "track") {
    const albumId = trackRef.value?.item?.album_id;
    return albumId ? [formatImageUrl(albumId)] : [];
  }
  return [];
});
</script>

<style scoped>
.steeringArtwork {
  position: relative;
  flex: 0 0 auto;
  overflow: hidden;
  background: linear-gradient(135deg, #2a2f33, #16191b);
  box-shadow: var(--shadow-md);
}

.size-xs {
  width: 32px;
  height: 32px;
}

.size-sm {
  width: 48px;
  height: 48px;
}

.size-md {
  width: 72px;
  height: 72px;
}

.size-lg {
  width: 100%;
  aspect-ratio: 1;
}

.square {
  border-radius: var(--radius-sm);
}

.size-md.square,
.size-lg.square {
  border-radius: var(--radius-lg);
}

.round {
  border-radius: 50%;
}

.image {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: cover;
}

/* Transparent rather than hidden: the image only starts loading once visible. */
.image:not([src]) {
  opacity: 0;
}

.fallback {
  position: absolute;
  inset: 0;
  display: grid;
  place-items: center;
  color: var(--text-subdued);
  font-weight: var(--font-black);
  font-size: 1.1em;
}

.size-lg .fallback {
  font-size: 3rem;
}

.concept {
  background: none;
}

.conceptLabel {
  position: absolute;
  top: 10%;
  left: 10%;
  right: 10%;
  color: #fff;
  font-size: clamp(0.85rem, 2.2vw, 1.35rem);
  font-weight: var(--font-black);
  line-height: 1.1;
  letter-spacing: -0.01em;
  overflow-wrap: anywhere;
}

/* A tilted block in the corner, like Spotify's genre tiles. */
.conceptShape {
  position: absolute;
  right: -14%;
  bottom: -10%;
  width: 52%;
  height: 52%;
  border-radius: var(--radius-md);
  background: rgba(0, 0, 0, 0.22);
  transform: rotate(25deg);
  box-shadow: -6px 6px 18px rgba(0, 0, 0, 0.25);
}

.size-xs .conceptShape,
.size-sm .conceptShape {
  right: -22%;
  bottom: -22%;
  width: 70%;
  height: 70%;
}
</style>
