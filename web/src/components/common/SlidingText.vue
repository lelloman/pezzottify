<template>
  <div class="sliding-container" ref="containerRef" @click.stop="$emit('click')">
    <!-- Primary slot with conditional styling -->
    <div ref="contentRef" :class="computeContentClasses">
      <p>
        <slot></slot>
      </p>
    </div>

    <!-- Duplicate slot for animation that uses the same content -->
    <div v-if="shouldAnimate" aria-hidden="true" :class="computeContentClasses">
      <slot></slot>
    </div>
  </div>
</template>

<script setup>
import { computed, defineProps, defineEmits, ref, onMounted, onUpdated, nextTick, onBeforeUnmount } from 'vue';

const props = defineProps({
  infiniteAnimation: {
    type: Boolean,
    default: false,
  },
  hoverAnimation: {
    type: Boolean,
    default: false,
  }
});

const containerRef = ref(null);
const contentRef = ref(null);
const isOverflowing = ref(false);

// Check if content is overflowing
const checkOverflow = async () => {
  await nextTick();
  if (containerRef.value && contentRef.value) {
    const containerWidth = containerRef.value.offsetWidth;
    const contentWidth = contentRef.value.scrollWidth;
    isOverflowing.value = contentWidth > containerWidth;
  }
};

// Add debounce function to prevent too many resize checks
const debounce = (fn, delay) => {
  let timeout;
  return (...args) => {
    clearTimeout(timeout);
    timeout = setTimeout(() => fn(...args), delay);
  };
};

// Create debounced version of checkOverflow
const debouncedCheckOverflow = debounce(checkOverflow, 200);

// Should animate only if infiniteAnimation is true AND content is overflowing
const shouldAnimate = computed(() => {
  return (props.infiniteAnimation || props.hoverAnimation) && isOverflowing.value;
});

const computeContentClasses = computed(() => {
  return {
    'animating': props.infiniteAnimation && shouldAnimate.value,
    'non-animating': !props.infiniteAnimation && !props.hoverAnimation,
    'animateOnHover': props.hoverAnimation,
  };
});

// Check overflow on mount and updates, and set up resize listener
onMounted(() => {
  checkOverflow();
  window.addEventListener('resize', debouncedCheckOverflow);
});

// Clean up resize event listener when component is unmounted
onBeforeUnmount(() => {
  window.removeEventListener('resize', debouncedCheckOverflow);
});

onUpdated(checkOverflow);

defineEmits(['click']);
</script>

<style scoped>
.sliding-container {
  --gap: 2rem;
  display: flex;
  overflow: hidden;
  user-select: none;
  gap: var(--gap);
  width: 100%;
}

.animating {
  display: flex;
  animation: scroll 8s linear infinite;
  flex-shrink: 0;
  justify-content: space-around;
  gap: var(--gap);
}

.non-animating p {
  margin: 0;
  overflow: hidden;
  width: 100%;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.non-animating {
  width: 100%;
}

.animateOnHover {
  display: flex;
  flex-shrink: 0;
  justify-content: space-around;
  gap: var(--gap);
  width: 100%;
}

.animateOnHover p {
  margin: 0;
  overflow: hidden;
  width: 100%;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.animateOnHover:hover {
  display: flex;
  animation: scroll 8s linear infinite;
  flex-shrink: 0;
  justify-content: space-around;
  gap: var(--gap);
}

.animateOnHover:hover p {
  width: auto;
  overflow: visible;
  text-overflow: clip;
  white-space: nowrap;
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
