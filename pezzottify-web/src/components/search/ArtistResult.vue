<template>
  <div class="searchResultRow" :data-id="result" @click="handleClick(result)">
    <img :src="imageUrl" alt="Image" class="searchResultRoundImage" />
    <h3 class="title">{{ result.name }}</h3>
  </div>
</template>

<script setup>
import '@/assets/search.css'
import { computeImageUrl } from '@/utils';
import { useRouter } from 'vue-router';

const props = defineProps({
  result: {
    type: Object,
    required: true,
  }
});
const imageUrl = computeImageUrl(props.result.image_id);

const router = useRouter();

const handleClick = (event) => {
  console.log(event);
  router.push("/artist/" + event.id);
  //playerStore.setTrack(id);
}

</script>

<style scoped>
.title {
  margin: 0;
  font-size: 16px;
  font-weight: bold;
}
</style>
