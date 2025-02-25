<template>
  <div v-if="data">
    <div class="topSection">
      <MultiSourceImage class="coverImage" :urls="coverUrls" />
      <div class="artistInfoColum">
        <h1 class="artistName"> {{ data.name }}</h1>
        <div class="verticalFiller"></div>
        <ToggableFavoriteIcon :toggled="isArtistLiked" :clickCallback="handleClickOnFavoriteIcon" />
      </div>
    </div>
    <div v-if="data" class="relatedArtistsContainer">
      <LoadArtistListItem v-for="artistId in data.related" :key="artistId" :artistId="artistId" />
    </div>
    <div class="discographyContainer">
      <h1>Discography:</h1>
      <ArtistAlbumCards :artistId="artistId" />
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
import { useUserStore } from '@/store/user.js';
import MultiSourceImage from '@/components/common/MultiSourceImage.vue';
import ArtistAlbumCards from '@/components/common/ArtistAlbumCards.vue';
import ToggableFavoriteIcon from '@/components/common/ToggableFavoriteIcon.vue';
import LoadArtistListItem from '../common/LoadArtistListItem.vue';

const props = defineProps({
  artistId: {
    type: String,
    required: true,
  }
});

const data = ref(null);
const coverUrls = ref(null);
const isArtistLiked = ref(false);

const userStore = useUserStore();

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

watch([() => userStore.likedArtistsIds, data],
  ([likedArtis, artistData], [oldLikedArtists, oldArtistData]) => {
    if (likedArtis && artistData) {
      isArtistLiked.value = likedArtis.includes(props.artistId);
    }
  },
  { immediate: true }
);

const handleClickOnFavoriteIcon = () => {
  userStore.setArtistIsLiked(props.artistId, !isArtistLiked.value);
}

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
  userStore.triggerArtistsLoad();
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

.artistInfoColum {
  display: flex;
  flex-direction: column;
  margin: 0 16px;
}

.artistName {}

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

.verticalFiller {
  flex: 1;
}
</style>
