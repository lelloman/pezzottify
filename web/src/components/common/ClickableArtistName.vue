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
  color: var(--text-subdued);
}

.artistSpan:hover {
  text-decoration: underline;
  cursor: pointer;
  color: var(--text-base);
}
</style>
