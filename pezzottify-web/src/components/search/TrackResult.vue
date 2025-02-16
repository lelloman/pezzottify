<template>
  <div class="searchResultRow" :data-id="result" @click="handleTrackClick(result)">
    <MultiSourceImage :urls="[imageUrl]" alt="Image" class="searchResultImage" />
    <div class="column">
      <h3 class="title">{{ result.name }}</h3>
      <ClickableArtistsNames :artistsIdsNames="result.artists_ids_names" />
    </div>
    <h3 class="duration">{{ duration }}</h3>
    <PlayIcon class="searchResultPlayIcon" :data-id="result" @click.stop="handlePlayClick(result)" />
  </div>
</template>


<script setup>
import '@/assets/search.css'
import { computedImageUrl, formatDuration } from '@/utils';
import { usePlayerStore } from '@/store/player';
import { useRouter } from 'vue-router';
import PlayIcon from '../icons/PlayIcon.vue';
import ClickableArtistsNames from '../common/ClickableArtistsNames.vue';
import MultiSourceImage from '../common/MultiSourceImage.vue';

const props = defineProps({
  result: {
    type: Object,
    required: true,
  }
});
const imageUrl = computedImageUrl(props.result.image_id);


const duration = formatDuration(props.result.duration);

const playerStore = usePlayerStore();
const router = useRouter();

const handleTrackClick = (event) => {
  console.log("trackClick");
  console.log(event);
  router.push("/track/" + event.id);
}

const handlePlayClick = (event) => {
  console.log("play click");
  console.log(event);
  playerStore.setTrack(event);
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
