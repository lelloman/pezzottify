<template>
  <div v-if="data">
    <div class="topSection">
      <MultiSourceImage class="coverImage" :urls="coverUrls" />
      <div class="artistInfoColum">
        <h1 class="artistName"> {{ data.name }}</h1>
      </div>
    </div>
    <div v-if="data" class="relatedArtistsContainer">
      <RelatedArtist v-for="artistId in data.related" :key="artistId" :artistId="artistId" />
    </div>
    <div class="discographyContainer">
      <h1>Discography:</h1>
      <ArtistAlbums :artistId="artistId" />
    </div>
  </div>

  <div v-else>
    <p>Loading {{ artistId }}...</p>
  </div>
</template>

<script setup>
import { ref, watch, onMounted } from 'vue';
import axios from 'axios';
import { chooseArtistCoverImageUrl } from '@/utils';
import MultiSourceImage from '@/components/common/MultiSourceImage.vue';
import RelatedArtist from '@/components/common/LoadArtistListItem.vue';
import ArtistAlbums from '../common/ArtistAlbums.vue';

const props = defineProps({
  artistId: {
    type: String,
    required: true,
  }
});

const data = ref(null);
const coverUrls = ref(null);

const fetchData = async (id) => {
  if (!id) return;
  data.value = null;
  try {
    const response = await axios.get(`/v1/content/artist/${id}`);
    data.value = response.data;
  } catch (error) {
    console.error('Error fetching data:', error);
  }
};

watch(data,
  (newData) => {
    if (newData) {
      coverUrls.value = chooseArtistCoverImageUrl(newData);
    }
  },
  { immediate: true }
);

watch(() => props.artistId, (newId) => {
  fetchData(newId);
});

onMounted(() => {
  fetchData(props.artistId);
});
</script>

<style scoped>
.topSection {
  display: flex;
  flex-direction: row;
}

.coverImage {
  width: 400px;
  height: 400;
  object-fit: contain
}

.artistName {
  margin: 0 16px;

}

.relatedArtistsContainer {
  width: 100%;
  display: flex;
  flex-direction: row;
  overflow-x: auto;
  margin: 16px;
}

.discographyContainer {
  margin: 16px;
}
</style>
