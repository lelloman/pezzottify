<template>
  <div class="container">
    <EmptyFavoriteIcon class="favIcon" v-if="!toggled" @click.stop="props.clickCallback" />
    <FilledFavoriteIcon class="favIcon filled" v-else @click.stop="clickCallback" />
  </div>
</template>

<script setup>
import { ref, defineProps, watch } from 'vue';
import EmptyFavoriteIcon from '../icons/EmptyFavoriteIcon.vue';
import FilledFavoriteIcon from '../icons/FilledFavoriteIcon.vue';

const props = defineProps({
  toggled: {
    type: Boolean,
    required: true,
  },
  clickCallback: {
    type: Function,
    required: true,
  }
});
</script>

<style scoped>
.container {
  fill: var(--accent-color);
}

.favIcon {
  fill: white;
  cursor: pointer;
  scale: 0.9;
  transition: scale 0.3s ease;
}

.favIcon:hover {
  scale: 1;
  transition: scale 0.3s ease;
}

.favIcon:active {
  scale: 0.8;
  transition: scale 0.3s ease;
}

.filled {
  fill: var(--accent-color) !important;
}
</style>
