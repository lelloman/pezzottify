<template>
  <div v-if="isLoading">...</div>
  <ClickableArtistsNames v-else-if="artistsIdsNames" :prefix="prefix" :artistsIdsNames="artistsIdsNames" />
</template>

<script setup>
import { onMounted, ref } from 'vue';
import axios from 'axios';
import ClickableArtistsNames from './ClickableArtistsNames.vue';

const props = defineProps({
  prefix: {
    type: String,
  },
  artistsIds: {
    type: Array,
    required: true,
  }
});

const isLoading = ref(false);
const artistsIdsNames = ref(null);

onMounted(async () => {
  isLoading.value = true;
  const artistsPromises = props.artistsIds.map(async (artistId) => {
    const response = await axios.get(`/v1/content/artist/${artistId}`);
    return [artistId, response.data.name];
  });
  console.log("LoadClickableArtistsName starting to wait...");
  artistsIdsNames.value = await Promise.all(artistsPromises);
  console.log("LoadClickableArtistsName starting to Waited");
  isLoading.value = false;
});

</script>
