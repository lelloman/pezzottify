<template>
  <div class="relatedArtistWrapper">
    <div v-if="loading">Loading...</div>
    <div v-else-if="artistData" class="searchResultRow" :data-id="artistData.id" @click="handleClick(artistData)">
      <MultiSourceImage :urls="chooseSmallArtistImageUrl(artistData)" class="searchResultRoundImage" />
      <h3 class="title">{{ artistData.name }}</h3>
    </div>
    <div v-else-if="error">Error. {{ error }}</div>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue';
import { useRouter } from 'vue-router';
import { chooseSmallArtistImageUrl } from '@/utils';
import axios from 'axios';
import MultiSourceImage from './MultiSourceImage.vue';

const props = defineProps({
  artistId: {
    type: String,
    required: true,
  }
});

const artistData = ref(null);
const loading = ref(true);
const error = ref(null);
const router = useRouter();

const fetchArtistData = async (id) => {
  try {
    const response = await axios.get(`/v1/content/artist/${id}`);
    artistData.value = response.data;
  } catch (err) {
    error.value = err.message;
  } finally {
    loading.value = false;
  }
};

onMounted(() => {
  fetchArtistData(props.artistId);
});

const handleClick = (artist) => {
  router.push("/artist/" + artist.id);
};
</script>

<style scoped>
.relatedArtistWrapper {
  min-width: 200px;
  margin: 10px;
  height: 100%;
  align-content: center;
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
