<template>
  <div class=".albumWrapper">
    <div v-if="loading">Loading...</div>
    <div v-else-if="album" class="searchResultRow" :data-id="album.id" @click="handleClick(album.id)">
      <MultiSourceImage :urls="chooseAlbumCoverImageUrl(album)" class="searchResultImage scaleClickFeedback" />
      <div class="column">
        <h3 class="title">{{ album.name }}</h3>
        <LoadClickableArtistsNames v-if="showArtists" class="artistsNames" :artistsIds="album.artists_ids" />
      </div>

      <PlayIcon class="searchResultPlayIcon scaleClickFeedback bigIcon" :data-id="album.id"
        @click.stop="handlePlayClick(album.id)" />
    </div>
    <div v-else-if="error">Error. {{ error }}</div>
  </div>
</template>

<script setup>
import '@/assets/base.css'
import '@/assets/search.css'
import '@/assets/icons.css'
import { ref, watch } from 'vue';
import { useRouter } from 'vue-router';
import { chooseAlbumCoverImageUrl } from '@/utils';
import MultiSourceImage from './MultiSourceImage.vue';
import PlayIcon from '@/components/icons/PlayIcon.vue';
import { usePlayerStore } from '@/store/player';
import LoadClickableArtistsNames from '@/components/common/LoadClickableArtistsNames.vue';
import { useStaticsStore } from '@/store/statics';

const router = useRouter();
const staticsStore = useStaticsStore();
const playerStore = usePlayerStore();

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

const album = ref(null);
const artistsRefs = ref(null);
const loading = ref(true);
const error = ref(null);


watch(staticsStore.getAlbum(props.albumId), (newData) => {
  loading.value = newData && newData.loading;
  if (newData && newData.item && typeof newData.item === 'object') {
    artistsRefs.value = newData.item.artists_ids.map((artistId) => staticsStore.getArtist(artistId));
    album.value = newData.item;
  }
}, { immediate: true });


const handlePlayClick = (event) => {
  playerStore.setAlbumId(event);
  playerStore.setIsPlaying(true);
}

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
