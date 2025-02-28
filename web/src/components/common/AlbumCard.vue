<template>
  <div class=".albumWrapper">
    <div v-if="loading">Loading...</div>
    <div v-else-if="albumData" class="searchResultRow" :data-id="albumData.id" @click="handleClick(albumData.id)">
      <MultiSourceImage :urls="chooseAlbumCoverImageUrl(albumData)" class="searchResultImage scaleClickFeedback" />
      <div class="column">
        <h3 class="title">{{ albumData.name }}</h3>
        <ClickableArtistsNames v-if="showArtists" class="artistsNames" :artistsIdsNames="artistsIdsNames" />
      </div>

      <PlayIcon class="searchResultPlayIcon scaleClickFeedback bigIcon" :data-id="albumData.id"
        @click.stop="handlePlayClick(albumData.id)" />
    </div>
    <div v-else-if="error">Error. {{ error }}</div>
  </div>
</template>

<script setup>
import '@/assets/base.css'
import '@/assets/search.css'
import '@/assets/icons.css'
import { ref, onMounted } from 'vue';
import { useRouter } from 'vue-router';
import { chooseAlbumCoverImageUrl } from '@/utils';
import MultiSourceImage from './MultiSourceImage.vue';
import PlayIcon from '@/components/icons/PlayIcon.vue';
import { usePlayerStore } from '@/store/player';
import { useRemoteStore } from '@/store/remote';
import ClickableArtistsNames from './ClickableArtistsNames.vue';

const props = defineProps({
  albumId: {
    type: String,
    required: true,
  },
  showArtists: {
    type: Boolean,
    required: false,
    withDefaults: false,
  }
});

const albumData = ref(null);
const artistsIdsNames = ref([]);
const loading = ref(true);
const error = ref(null);
const router = useRouter();

const playerStore = usePlayerStore();
const remoteStore = useRemoteStore();

const handlePlayClick = (event) => {
  console.log("play click");
  console.log(event);
  playerStore.setAlbumId(event);
  playerStore.setIsPlaying(true);
}

const fetchAlbumData = async (id) => {

  const responseData = await remoteStore.fetchResolvedAlbum(id);

  loading.value = false;
  if (responseData) {
    artistsIdsNames.value = responseData.album.artists_ids.map((artistId) => {
      return [artistId, responseData.artists[artistId].name];
    });
    albumData.value = responseData.album;
  } else {
    error.value = "Error fetching album data";
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

.column {
  flex: 1;
  display: flex;
  flex-direction: column;
}
</style>
