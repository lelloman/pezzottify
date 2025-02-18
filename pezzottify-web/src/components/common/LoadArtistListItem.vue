<template>
  <div class="relatedArtistWrapper">
    <div v-if="loading">Loading...</div>
    <ArtistListItem v-else-if="artistData" :data-id="artistData.id" :artist="artistData" />
    <div v-else-if="error">Error. {{ error }}</div>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue';
import axios from 'axios';
import ArtistListItem from '@/components/common/ArtistListItem.vue';

const props = defineProps({
  artistId: {
    type: String,
    required: true,
  }
});

const artistData = ref(null);
const loading = ref(true);
const error = ref(null);

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

</script>

<style scoped>
.relatedArtistWrapper {
  min-width: 200px;
  margin: 10px;
  height: 100%;
  align-content: center;
}
</style>
