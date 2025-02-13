<template>
  Showing track {{ trackId }}
</template>

<script setup>

defineProps({
  trackId: {
    type: String,
    required: true,
  }
});
</script>
