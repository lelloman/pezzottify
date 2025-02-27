<template>
  <div class="container">
    <EmptyFavoriteIcon class="favIcon scaleClickFeedback bigIcon" v-if="!toggled" @click.stop="props.clickCallback" />
    <FilledFavoriteIcon class="favIcon filled scaleClickFeedback bigIcon" v-else @click.stop="clickCallback" />
  </div>
</template>

<script setup>
import { defineProps } from 'vue';
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
@import "@/assets/icons.css";

.container {
  fill: var(--accent-color);
}

.favIcon {
  fill: white;
  cursor: pointer;
}

.filled {
  fill: var(--accent-color) !important;
}
</style>
