<template>
  <div v-if="albumIds" class="albumsContainer">
    <ArtistAlbum v-for="albumId in albumIds" :key="albumId" :albumId="albumId" />
  </div>
  <div v-else-if="isLoading">
    Loading...
  </div>
  <div v-else>
    {{ error }}
  </div>
</template>

<script setup>
import { onMounted, ref } from 'vue';
import axios from 'axios';
import ArtistAlbum from './ArtistAlbum.vue';

const props = defineProps({
  artistId: {
    type: String,
    required: true,
  }
});

const albumIds = ref(null);
const error = ref(null);
const isLoading = ref(false);

const loadAlbumIds = async (artistId) => {
  isLoading.value = true;
  try {
    const response = await axios.get(`/v1/content/artist/${artistId}/albums`);
    albumIds.value = response.data;
  } catch (err) {
    error.value = err.message;
  } finally {
    isLoading.value = false;
  }
};

onMounted(() => {
  loadAlbumIds(props.artistId);
})

</script>

<style scoped>
.albumsContainer {
  display: grid;
  gap: 16px;
  grid-template-columns: repeat(1, 1fr);
}

@media (min-width: 1000px) {
  .albumsContainer {
    grid-template-columns: repeat(2, 1fr);
  }
}

@media (min-width: 1500px) {
  .albumsContainer {
    grid-template-columns: repeat(3, 1fr);
  }
}
</style>
