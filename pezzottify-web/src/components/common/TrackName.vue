<template>
  <div :data-id="track.id" @click.stop="handleClick">
    <h3 class="trackName">{{ track.name }}</h3>
  </div>
</template>

<script setup>
import { useRouter } from 'vue-router';

const props = defineProps({
  track: {
    type: Object,
    required: true,
  }
});

const router = useRouter();

const handleClick = () => {
  router.push("/track/" + props.track.id);
};
</script>

<style scoped>
.trackName {
  margin: 0;
  font-size: 16px;
  font-weight: bold;
}

.trackName:hover {
  cursor: pointer;
  text-decoration: underline;
}
</style>
