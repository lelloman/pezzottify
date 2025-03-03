<template>
  <div v-if="artist">
    <div class="topSection">
      <MultiSourceImage class="coverImage" :urls="coverUrls" />
      <div class="artistInfoColum">
        <h1 class="artistName"> {{ artist.name }}</h1>
        <div class="verticalFiller"></div>
        <ToggableFavoriteIcon :toggled="isArtistLiked" :clickCallback="handleClickOnFavoriteIcon" />
      </div>
    </div>
    <div class="relatedArtistsContainer">
      <LoadArtistListItem v-for="artistId in artist.related" :key="artistId" :artistId="artistId" />
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
import { chooseArtistCoverImageUrl } from '@/utils';
import { useUserStore } from '@/store/user.js';
import { useStaticsStore } from '@/store/statics.js';
import MultiSourceImage from '@/components/common/MultiSourceImage.vue';
import ArtistAlbumCards from '@/components/common/ArtistAlbumCards.vue';
import ToggableFavoriteIcon from '@/components/common/ToggableFavoriteIcon.vue';
import LoadArtistListItem from '@/components/common/LoadArtistListItem.vue';

const props = defineProps({
  artistId: {
    type: String,
    required: true,
  }
});

const artist = ref(null);
const coverUrls = ref(null);
const isArtistLiked = ref(false);
const userStore = useUserStore();
const staticsStore = useStaticsStore();

let artistDataUnwatcher = null;

const fetchData = async (id) => {
  if (artistDataUnwatcher) {
    artistDataUnwatcher();
    artistDataUnwatcher = null;
  }
  if (!id) return;

  artistDataUnwatcher = watch(
    staticsStore.getArtist(id),
    (newData) => {
      if (newData && newData.item && typeof newData.item === 'object') {
        coverUrls.value = chooseArtistCoverImageUrl(newData.item);
        artist.value = newData.item;
      }
    },
    { immediate: true });
};

watch([() => userStore.likedArtistsIds, artist],
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
