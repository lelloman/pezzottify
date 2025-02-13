<template>
  Showing artist {{ artistId }}
</template>

<script setup>

defineProps({
  artistId: {
    type: String,
    required: true,
  }
});
</script>
