<template>
  <span class="artistSpan" @click.stop="handleClick">{{ artistName }}</span>
</template>

<script setup>
import { useRouter } from 'vue-router';

const props = defineProps({
  artistId: {
    type: String,
    required: true,
  },
  artistName: {
    type: String,
    required: true,
  }
})

const router = useRouter();

function handleClick() {
  router.push("/artist/" + props.artistId);
}
</script>

<style scoped>
.artistSpan {
  color: inherit;
}

.artistSpan:hover {
  text-decoration: underline;
  cursor: pointer;
}
</style>
