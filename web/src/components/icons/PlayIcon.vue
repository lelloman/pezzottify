<template>
  <svg viewBox="0 0 24 24">
    <g>
      <circle cx="12" cy="12" r="6" :fill="props.triangleColor" />
    </g>
    <g>
      <path
        :fill="props.circleColor"
        d="M12,2C6.48,2,2,6.48,2,12s4.48,10,10,10s10-4.48,10-10S17.52,2,12,2z M9.5,16.5v-9l7,4.5L9.5,16.5z"
      />
    </g>
  </svg>
</template>

<script setup>
const props = defineProps({
  triangleColor: {
    type: String,
    default: "white",
  },
  circleColor: {
    type: String,
    default: "var(--accent-color)",
  },
});
</script>
