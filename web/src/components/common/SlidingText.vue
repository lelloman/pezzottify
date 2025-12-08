<template>
  <div
    class="sliding-container"
    ref="containerRef"
    :style="containerStyle"
    @click.stop="$emit('click')"
    @mouseenter="handleHover(true)"
    @mouseleave="handleHover(false)"
  >
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
import {
  computed,
  defineProps,
  defineEmits,
  ref,
  onMounted,
  onUpdated,
  nextTick,
  onBeforeUnmount,
} from "vue";

const props = defineProps({
  infiniteAnimation: {
    type: Boolean,
    default: false,
  },
  hoverAnimation: {
    type: Boolean,
    default: false,
  },
  scrollSpeed: {
    type: Number,
    default: 50, // pixels per second
  },
});

const containerRef = ref(null);
const contentRef = ref(null);
const isOverflowing = ref(false);
const animationDuration = ref(8); // default duration in seconds
const isHovering = ref(false); // Track hover state

// Handle mouse hover events
const handleHover = (hovering) => {
  isHovering.value = hovering;
  if (props.hoverAnimation && hovering) {
    // Force recalculation when hovering starts
    checkOverflow();
  }
};

// Check if content is overflowing and calculate animation duration
const checkOverflow = async () => {
  await nextTick();
  if (containerRef.value && contentRef.value) {
    const containerWidth = containerRef.value.offsetWidth;
    const contentWidth = contentRef.value.scrollWidth;
    isOverflowing.value = contentWidth > containerWidth;

    // Calculate animation duration based on content width and desired scroll speed
    // We need to scroll the full content width + gap
    if (isOverflowing.value) {
      // Content width + gap, converted to seconds based on scroll speed
      animationDuration.value =
        (contentWidth +
          parseFloat(
            getComputedStyle(containerRef.value).getPropertyValue("--gap"),
          )) /
        props.scrollSpeed;
    }
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

// Should animate only if animation is enabled
const shouldAnimate = computed(() => {
  return (
    isOverflowing.value &&
    (props.infiniteAnimation || (props.hoverAnimation && isHovering.value))
  );
});

const computeContentClasses = computed(() => {
  const animating =
    isOverflowing.value &&
    (props.infiniteAnimation || (props.hoverAnimation && isHovering.value));
  const ellipsisOverflow = !props.infiniteAnimation && !props.hoverAnimation;
  const animateOnHover = props.hoverAnimation && !isHovering.value;
  const hovering =
    isOverflowing.value && props.hoverAnimation && isHovering.value;
  const base = !(animating || ellipsisOverflow || animateOnHover || hovering);
  return {
    animating: animating,
    "ellipsis-overflow": ellipsisOverflow,
    animateOnHover: animateOnHover,
    hovering: hovering,
    base: base,
  };
});

// IMPORTANT CHANGE: Move the animation style to the container level
// This ensures the CSS variable is available to all child elements including hover states
const containerStyle = computed(() => {
  return { "--animation-duration": `${animationDuration.value}s` };
});

// Check overflow on mount and updates, and set up resize listener
onMounted(() => {
  checkOverflow();
  window.addEventListener("resize", debouncedCheckOverflow);
});

// Clean up resize event listener when component is unmounted
onBeforeUnmount(() => {
  window.removeEventListener("resize", debouncedCheckOverflow);
});

onUpdated(checkOverflow);

defineEmits(["click"]);
</script>

<style scoped>
.sliding-container {
  --gap: 2rem;
  display: flex;
  overflow: hidden;
  user-select: none;
  gap: var(--gap);
  width: 100%;
  --animation-duration: 8s;
  /* Default, will be overridden by dynamic calculation */
}

.animating {
  display: flex;
  animation: scroll var(--animation-duration) linear infinite;
  flex-shrink: 0;
  justify-content: space-around;
  gap: var(--gap);
}

.base {
  width: 100%;
}

.base p {
  width: 100%;
}

.ellipsisOverflow p {
  margin: 0;
  overflow: hidden;
  width: 100%;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ellipsisOverflow {
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

.hovering {
  display: flex;
  animation: scroll var(--animation-duration) linear infinite;
  flex-shrink: 0;
  justify-content: space-around;
  gap: var(--gap);
}

.hovering p {
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
