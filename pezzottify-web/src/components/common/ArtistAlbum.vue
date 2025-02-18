<template>
  <div class=".albumWrapper">
    <div v-if="loading">Loading...</div>
    <div v-else-if="albumData" class="searchResultRow" :data-id="albumData.id" @click="handleClick(albumData.id)">
      <MultiSourceImage :urls="chooseAlbumCoverImageUrl(albumData)" class="searchResultRoundImage" />
      <h3 class="title">{{ albumData.name }}</h3>

      <PlayIcon class="searchResultPlayIcon" :data-id="albumData.id" @click.stop="handlePlayClick(albumData.id)" />
    </div>
    <div v-else-if="error">Error. {{ error }}</div>
  </div>
</template>

<script setup>
import '@/assets/search.css'
import { ref, onMounted } from 'vue';
import { useRouter } from 'vue-router';
import { chooseAlbumCoverImageUrl } from '@/utils';
import axios from 'axios';
import MultiSourceImage from './MultiSourceImage.vue';
import PlayIcon from '@/components/icons/PlayIcon.vue';
import { usePlayerStore } from '@/store/player';

const props = defineProps({
  albumId: {
    type: String,
    required: true,
  }
});

const albumData = ref(null);
const loading = ref(true);
const error = ref(null);
const router = useRouter();

const playerStore = usePlayerStore();

const handlePlayClick = (event) => {
  console.log("play click");
  console.log(event);
  playerStore.setAlbumId(event);
  playerStore.setIsPlaying(true);
}

const fetchAlbumData = async (id) => {
  try {
    const response = await axios.get(`/v1/content/album/${id}`);
    albumData.value = response.data;
  } catch (err) {
    error.value = err.message;
  } finally {
    loading.value = false;
  }
};

onMounted(() => {
  fetchAlbumData(props.albumId);
});

const handleClick = (albumId) => {
  router.push("/album/" + albumId);
};
</script>

<style scoped>
.relatedAlbumWrapper {
  max-width: 400px;
}

.searchResultRoundImage {
  width: 80px;
  height: 80px;
  border-radius: 40px;
  margin-right: 10px;
}

.title {
  margin: 0;
  font-size: 16px;
  font-weight: bold;
}
</style>
