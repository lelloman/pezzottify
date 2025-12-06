<template>
  <p class="subtitle">
    <span v-if="prefix">{{ prefix }}</span>
    <span v-for="(artist, index) in artistsIdsNames" :key="artist">
      <span v-if="index > 0">, </span>
      <ClickableArtistName :href="'/artist/' + artist[0]" :artistId="artist[0]" :artistName="artist[1]" />
    </span>
  </p>
</template>

<script setup>
import ClickableArtistName from './ClickableArtistName.vue';

defineProps({
  prefix: {
    type: String,
  },
  artistsIdsNames: {
    type: Array,
    required: true,
  }
})
</script>

<style scoped>
.subtitle {
  margin: 0;
  font-size: 14px;
  color: inherit;
}
</style>
