<template>
  <div class="searchResultRow" :data-id="result" @click="handleClick(result)">
    <img :src="imageUrl" alt="Image" class="searchResultImage" />
    <div class="column">
      <h3 class="title">{{ result.name }}</h3>
      <p class="subtitle">{{ result.year }} - {{ result.artists_names.join(", ") }}</p>
    </div>
  </div>
</template>

<script setup>
import '@/assets/search.css'
import { computeImageUrl } from '@/utils.js';
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
  router.push("/album/" + event.id);
  //playerStore.setTrack(id);
}
</script>

<style scoped>
.column {
  display: flex;
  flex-direction: column;
}

.title {
  margin: 0;
  font-size: 16px;
  font-weight: bold;
}

.subtitle {
  margin: 0;
  font-size: 14px;
  color: #666;
}
</style>
