<template>
  <div v-if="albumIds && albumIds.length > 0">
    <h1>Albums:</h1>
    <div class="albumsContainer">
      <AlbumCard v-for="albumId in albumIds" :key="albumId" :albumId="albumId" />
    </div>
  </div>
  <div v-if="featuresIds && featuresIds.length > 0">
    <h1>Features:</h1>
    <div class="albumsContainer">
      <AlbumCard v-for="albumId in featuresIds" :key="albumId" :albumId="albumId" />
    </div>
  </div>
  <div v-else-if="isLoading">
    Loading...
  </div>
  <div v-else>
    {{ error }}
  </div>
</template>

<script setup>
import { onMounted, watch, ref } from 'vue';
import AlbumCard from '@/components/common/AlbumCard.vue';
import { useRemoteStore } from '@/store/remote';

const props = defineProps({
  artistId: {
    type: String,
    required: true,
  }
});

const remoteStore = useRemoteStore();
const albumIds = ref(null);
const featuresIds = ref(null);
const error = ref(null);
const isLoading = ref(false);

const loadAlbumIds = async (artistId) => {
  isLoading.value = true;
  const artistsAlbumsResponse = await remoteStore.fetchArtistDiscography(artistId);
  if (artistsAlbumsResponse) {
    // The API now returns Album objects, not just IDs, so extract the IDs
    albumIds.value = artistsAlbumsResponse.albums ? artistsAlbumsResponse.albums.map(a => a.id) : [];
    featuresIds.value = artistsAlbumsResponse.features ? artistsAlbumsResponse.features.map(a => a.id) : [];
  } else {
    error.value = "Error fetching artist albums";
  }
  isLoading.value = false;
};

onMounted(() => {
  watch(() => props.artistId, () => {
    loadAlbumIds(props.artistId);
  }, { immediate: true });
})

</script>

<style scoped>
.albumsContainer {
  display: grid;
  gap: 16px;
  grid-template-columns: repeat(1, 1fr);
  justify-items: start;
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
