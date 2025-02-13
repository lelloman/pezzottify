<template>
  Showing album {{ albumId }}
</template>

<script setup>

defineProps({
  albumId: {
    type: String,
    required: true,
  }
});
</script>
