<template>
  <div class="searchResultRow" :data-id="result.id" @click="handleClick($event)">
    <img :src="imageUrl" alt="Image" class="searchResultImage" />
    <div class="column">
      <h3 class="title">{{ result.name }}</h3>
      <p class="subtitle">{{ result.artists_names.join(", ") }}</p>
    </div>
    <h3 class="duration">{{ duration }}</h3>
  </div>
</template>


<script setup>
import '@/assets/search.css'
import { computeImageUrl } from '@/utils';
import { usePlayerStore } from '@/store/player';

const props = defineProps({
  result: {
    type: Object,
    required: true,
  }
});
const imageUrl = computeImageUrl(props.result.image_id);
function formatDuration(d) {
  const seconds = Math.round(d / 1000);
  const pad = (num) => String(num).padStart(2, '0');
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const secs = seconds % 60;
  return `${pad(hours)}:${pad(minutes)}:${pad(secs)}`;
}
const duration = formatDuration(props.result.duration);

const playerStore = usePlayerStore();

const handleClick = (event) => {
  const id = event.currentTarget.dataset.id;
  console.log('Clicked item with ID:', id);
  playerStore.setTrack(id);
}

</script>

<style scoped>
.column {
  display: flex;
  flex-direction: column;
  flex: 1;
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

.duration {
  text-align: center;
  vertical-align: middle;
  height: 100%;
}
</style>
