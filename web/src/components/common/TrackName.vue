<template>
  <div class="nameContainer" :data-id="track.id" @click.stop="handleClick">
    <h3 :class="computeClass()" :title="track.name">{{ sanitizedTrackName }}</h3>
    <h3 v-if="props.infiniteAnimation" aria-hidden="true" :class="computeClass()" :title="track.name">{{
      sanitizedTrackName }}</h3>
  </div>
</template>

<script setup>
import { useRouter } from 'vue-router';
import { computed, ref } from 'vue';

const props = defineProps({
  track: {
    type: Object,
    required: true,
  },
  infiniteAnimation: {
    type: Boolean,
    default: false,
  },
});

const router = useRouter();

const sanitizedTrackName = computed(() => {
  // Replace HTML line breaks and remove any HTML tags
  return props.track.name
    .replace(/<br\s*\/?>/gi, ' ')  // Replace <br> tags with spaces
    .replace(/\r?\n|\r/g, ' ')     // Replace line feeds with spaces
    .replace(/<[^>]*>/g, '');      // Remove any remaining HTML tags
});

const handleClick = () => {
  router.push("/track/" + props.track.id);
};

const computeClass = () => {
  return {
    animatingText: props.infiniteAnimation,
    nonAnimatingText: props.infiniteAnimation === false || props.infiniteAnimation === undefined,
    trackName: true,
  };
};
</script>

<style scoped>
.nameContainer {
  --gap: 4rem;
  display: flex;
  overflow: hidden;
  user-select: none;
  gap: var(--gap);
}

.trackName {
  font-size: 16px;
  font-weight: bold;
  cursor: pointer;
  white-space: nowrap;
  overflow: hidden;

}

.trackName:hover {
  text-decoration: underline;
}

.nonAnimatingText {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.animatingText {
  display: flex;
  animation: scroll 8s linear infinite;
  flex-shrink: 0;
  justify-content: space-around;
  width: 100%;
  gap: var(--gap);
}

@keyframes scroll {
  from {
    transform: translateX(0);
  }

  to {
    transform: translateX(calc(-100% - var(--gap)));
  }
}
</style>
