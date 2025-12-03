<template>
  <SlidingText :infiniteAnimation="props.infiniteAnimation" :animateOnHover="props.animateOnHover" :data-id="track.id"
    @click="handleClick">
    <span class="track-name">{{ sanitizedTrackName }}</span>
  </SlidingText>
</template>

<script setup>
import { useRouter } from 'vue-router';
import { computed } from 'vue';
import SlidingText from './SlidingText.vue';

const props = defineProps({
  track: {
    type: Object,
    required: true,
  },
  infiniteAnimation: {
    type: Boolean,
    default: false,
  },
  animateOnHover: {
    type: Boolean,
    default: false,
  },
});

const router = useRouter();

const sanitizedTrackName = computed(() => {
  if (props.track.name) {
    return props.track.name
      .replace(/<br\s*\/?>/gi, ' ')  // Replace <br> tags with spaces
      .replace(/\r?\n|\r/g, ' ')     // Replace line feeds with spaces
      .replace(/<[^>]*>/g, '');      // Remove any remaining HTML tags
  } else {
    return '';
  }
});

const handleClick = () => {
  router.push("/track/" + props.track.id);
};
</script>

<style scoped>
.track-name {
  font-size: 16px;
  font-weight: bold;
  cursor: pointer;
  white-space: nowrap;
  width: 100%;
  color: var(--text-base);
}

.track-name:hover {
  text-decoration: underline;
}
</style>
