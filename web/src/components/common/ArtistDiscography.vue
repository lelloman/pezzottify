<template>
  <div v-if="albums && albums.length > 0">
    <h1>Albums:</h1>
    <div class="albumsContainer">
      <AlbumCard
        v-for="album in albums"
        :key="album.id"
        :album="album"
      />
    </div>
  </div>
  <div v-if="features && features.length > 0">
    <h1>Features:</h1>
    <div class="albumsContainer">
      <AlbumCard
        v-for="album in features"
        :key="album.id"
        :album="album"
      />
    </div>
  </div>
  <div v-else-if="isLoading">Loading...</div>
  <div v-else>
    {{ error }}
  </div>
</template>

<script setup>
import { onMounted, watch, ref } from "vue";
import AlbumCard from "@/components/common/AlbumCard.vue";
import { useRemoteStore } from "@/store/remote";

const props = defineProps({
  artistId: {
    type: String,
    required: true,
  },
});

const remoteStore = useRemoteStore();
const albums = ref(null);
const features = ref(null);
const error = ref(null);
const isLoading = ref(false);

// Sort albums by popularity (descending)
const sortByPopularity = (albumsList) => {
  return [...albumsList].sort((a, b) => (b.popularity || 0) - (a.popularity || 0));
};

const loadAlbums = async (artistId) => {
  isLoading.value = true;
  const artistsAlbumsResponse =
    await remoteStore.fetchArtistDiscography(artistId);
  if (artistsAlbumsResponse) {
    // Use album objects directly - no need to re-fetch, sorted by popularity
    albums.value = sortByPopularity(artistsAlbumsResponse.albums || []);
    features.value = sortByPopularity(artistsAlbumsResponse.features || []);
  } else {
    error.value = "Error fetching artist albums";
  }
  isLoading.value = false;
};

onMounted(() => {
  watch(
    () => props.artistId,
    () => {
      loadAlbums(props.artistId);
    },
    { immediate: true },
  );
});
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
