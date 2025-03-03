<template>
  <div class=".albumWrapper">
    <div v-if="loading">Loading...</div>
    <div v-else-if="album" class="searchResultRow" :data-id="album.id" @click="handleClick(album.id)">
      <MultiSourceImage :urls="chooseAlbumCoverImageUrl(album)" class="searchResultImage scaleClickFeedback" />
      <div class="column">
        <h3 class="title">{{ album.name }}</h3>
        <ClickableArtistsNames v-if="showArtists" class="artistsNames" :artistsIdsNames="artistsIdsNames" />
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
import { ref, onMounted, computed, watch } from 'vue';
import { useRouter } from 'vue-router';
import { chooseAlbumCoverImageUrl } from '@/utils';
import MultiSourceImage from './MultiSourceImage.vue';
import PlayIcon from '@/components/icons/PlayIcon.vue';
import { usePlayerStore } from '@/store/player';
import ClickableArtistsNames from './ClickableArtistsNames.vue';
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
const artistsIdsNames = ref([]);
const loading = ref(true);
const error = ref(null);


watch(() => staticsStore.getAlbum(props.albumId), (newData) => {
  console.log("AlbumCard got new AlbumData", newData.value);
  loading.value = newData.value && newData.value.loading;
  if (newData.value && typeof newData.value.item === 'object') {
    artistsRefs.value = newData.value.item.artists_ids.map((artistId) => staticsStore.getArtist(artistId));
    album.value = newData.value.item;
  }
}, { immediate: true });

watch(artistsRefs, (newRefs) => {
  console.log("AlbumCard got new ArtistsRefs", newRefs);
  if (newRefs) {
    if (newRefs.every((ref) => typeof ref.value.item === 'object')) {
      artistsIdsNames.value = newRefs.map((ref) => [ref.value.item.id, ref.value.item.name]);
      console.log("AlbumCard got new ArtistsRefs wrote artists ids names", artistsIdsNames.value);
    }
  }
}, { immediate: true });

const handlePlayClick = (event) => {
  console.log("play click");
  console.log(event);
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
