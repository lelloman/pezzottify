<template>
  <div v-if="data">
    <div class="topSection">
      <MultiSourceImage class="coverImage" :urls="coverUrls" />
      <div class="trackInfoColum">
        <h1 class="trackName">{{ track.name }} </h1>
        <h3>From: <span class="albumName" @click.stop="handleClickOnAlbumName">{{ album.name }} - {{
          getYearFromTimestamp(album.date) }}</span></h3>
        <p>Duration: {{ formatDuration(track.duration) }}</p>
        <p v-if="track.is_explicit">Explicit!</p>
        <p v-if="track.has_lyrics && track.language_of_performance.length"> Language: {{
          track.language_of_performance.join(", ")
          }}
        </p>
      </div>
    </div>
    <div class="commandsSection">
      <PlayIcon class="playTrackIcon scaleClickFeedback bigIcon" @click.stop="handleClickOnPlayTrack" />
    </div>
    <div class="artistsContainer">
      <LoadArtistListItem v-for="artistId in track.artists_ids" :key="artistId" :artistId="artistId" />
    </div>
  </div>
  <div v-else>
    <p>Loading {{ trackId }}...</p>
  </div>
</template>

<script setup>
import { ref, watch, onMounted } from 'vue';
import axios from 'axios';
import MultiSourceImage from '@/components/common/MultiSourceImage.vue';
import PlayIcon from '../icons/PlayIcon.vue';
import { usePlayerStore } from '@/store/player';
import { chooseAlbumCoverImageUrl, chooseAlbumCoverImageIds, formatDuration, getYearFromTimestamp } from '@/utils';
import { useRouter } from 'vue-router';
import LoadArtistListItem from '../common/LoadArtistListItem.vue';

const props = defineProps({
  trackId: {
    type: String,
    required: true,
  }
});

const data = ref(null);
const track = ref(null);
const album = ref(null);
const artists = ref(null);
const coverUrls = ref([]);

const router = useRouter();
const player = usePlayerStore();

const handleClickOnPlayTrack = () => {
  if (data.value) {
    const imagesIds = chooseAlbumCoverImageIds(album.value);
    player.setTrack({
      id: track.value.id,
      name: track.value.name,
      artists: artists.value.map((artist) => artist.name),
      image_id: imagesIds.length ? imagesIds[0] : null,
      duration: track.value.duration,
      albumId: track.value.album_id,
    });
  }
};

const handleClickOnAlbumName = () => {
  router.push("/album/" + album.value.id);
};

const fetchTrack = async (id) => {
  if (!id) return;
  data.value = null;
  try {
    const response = await axios.get(`/v1/content/track/${id}/resolved`);
    data.value = response.data;
  } catch (error) {
    console.error('Error fetching data:', error);
  }
};

watch(data, (newData) => {
  coverUrls.value = chooseAlbumCoverImageUrl(newData.album);
  track.value = data.value.tracks[props.trackId];
  album.value = data.value.album;
  artists.value = track.value.artists_ids.map((artistId) => newData.artists[artistId]);
})

watch(() => props.trackId, (newId) => {
  fetchTrack(newId);
});

onMounted(() => {
  fetchTrack(props.trackId);
});

</script>

<style scoped>
@import "@/assets/icons.css";

.topSection {
  display: flex;
  flex-direction: row;
}

.coverImage {
  width: 400px;
  height: 400;
  object-fit: contain
}

.trackInfoColum {
  margin: 0 16px;
  display: flex;
  flex-direction: column;
}

.albumName:hover {
  cursor: pointer;
  text-decoration: underline;
}

.playTrackIcon {
  fill: var(--accent-color);
  width: 64px;
  height: 64px;
}

.commandsSection {
  margin-top: 16px;
  margin-left: 8px;
  margin-right: 8px;
}

.artistsContainer {
  width: 100%;
  display: flex;
  flex-direction: row;
  overflow-x: auto;
}
</style>
