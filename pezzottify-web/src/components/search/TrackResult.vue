<template>
  <div class="searchResultRow" :data-id="result" @click="handleTrackClick(result)">
    <img :src="imageUrl" alt="Image" class="searchResultImage" />
    <div class="column">
      <h3 class="title">{{ result.name }}</h3>
      <p class="subtitle">{{ result.artists_names.join(", ") }}</p>
    </div>
    <h3 class="duration">{{ duration }}</h3>
    <svg class="playIcon" viewBox="0 0 24 24" :data-id="result" @click.stop="handlePlayClick(result)">
      <g>
        <circle cx="12" cy="12" r="6" fill="#000" />
      </g>
      <g>
        <path d="M12,2C6.48,2,2,6.48,2,12s4.48,10,10,10s10-4.48,10-10S17.52,2,12,2z M9.5,16.5v-9l7,4.5L9.5,16.5z" />
      </g>
    </svg>
  </div>
</template>


<script setup>
import '@/assets/search.css'
import { computeImageUrl } from '@/utils';
import { usePlayerStore } from '@/store/player';
import { useRouter } from 'vue-router';

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
const router = useRouter();

const handleTrackClick = (event) => {
  console.log("trackClick");
  console.log(event);
  router.push("/track/" + event.id);
  //playerStore.setTrack(id);
}

const handlePlayClick = (event) => {
  console.log("play click");
  console.log(event);
  playerStore.setTrack(event.id);
  playerStore.setIsPlaying(true);
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
