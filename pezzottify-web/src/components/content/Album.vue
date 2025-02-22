<template>
  <div v-if="data">
    <div class="topSection">
      <MultiSourceImage class="coverImage" :urls="coverUrls" />
      <div class="albumInfoColum">
        <h1 class="albumName"> {{ data.album.name }}</h1>
      </div>
    </div>
    <div class="commandsSection">
      <PlayIcon class="playAlbumIcon" @click.stop="handleClickOnPlayAlbum" />
      <ToggableFavoriteIcon :toggled="isAlbumLiked" :clickCallback="handleClickOnFavoriteIcon" />
    </div>
    <div class="artistsContainer">
      <RelatedArtist v-for="artistId in data.album.artists_ids" :key="artistId" :artistId="artistId" />
    </div>
    <div class="tracksContainer">
      <div v-for="(disc, discIndex) in data.album.discs" :key="disc" class="discContainer">
        <h1 v-if="data.album.discs.length > 1">Disc {{ discIndex + 1 }}<span v-if="disc.name">- {{ disc.name }}</span>
        </h1>
        <div v-for="(trackId, trackIndex) in disc.tracks" :key="trackId" class="track">
          <div :class="computeTrackRowClasses(trackId)"
            @click.stop="handleClickOnTrack(trackId, discIndex, trackIndex)">
            <div class="trackIndexSpan">
              <p>{{ trackIndex + 1 }} </p>
            </div>
            <TrackName :track="data.tracks[trackId]" class="trackNameSpan" />
            <div class="trackArtistsSpan">
              <ClickableArtistsNames
                :artistsIdsNames="data.tracks[trackId].artists_ids.map((artistId) => [artistId, data.artists[artistId].name])" />
            </div>
            <div class="trackDurationSpan">{{ formatDuration(data.tracks[trackId].duration) }}</div>
          </div>
        </div>
      </div>
    </div>
  </div>
  <div v-else>
    <p>Loading {{ albumId }}...</p>
  </div>
</template>

<script setup>
import { ref, watch, onMounted } from 'vue';
import axios from 'axios';
import { chooseAlbumCoverImageUrl, formatDuration } from '@/utils';
import MultiSourceImage from '@/components/common/MultiSourceImage.vue';
import RelatedArtist from '@/components/common/LoadArtistListItem.vue';
import TrackName from '../common/TrackName.vue';
import ClickableArtistsNames from '@/components/common/ClickableArtistsNames.vue';
import PlayIcon from '../icons/PlayIcon.vue';
import { usePlayerStore } from '@/store/player';
import { useUserStore } from '@/store/user';
import ToggableFavoriteIcon from '../common/ToggableFavoriteIcon.vue';

const props = defineProps({
  albumId: {
    type: String,
    required: true,
  }
});

const data = ref(null);
const coverUrls = ref(null);

const player = usePlayerStore();
const userStore = useUserStore();

const currentTrackId = ref(null);
const isAlbumLiked = ref(false);

const computeTrackRowClasses = (trackId) => {
  const isCurrentTrack = trackId == currentTrackId.value;
  return {
    trackRow: true,
    nonPlayingTrack: !isCurrentTrack,
    playingTrack: isCurrentTrack,
  };
}

watch(() => player.currentTrack,
  (newTrack) => {
    if (newTrack) {
      console.log("CurrentTrackId: " + newTrack.id);
      currentTrackId.value = newTrack.id;
    }
  },
  { immediate: true }
);

const fetchData = async (id) => {
  if (!id) return;
  data.value = null;
  try {
    const response = await axios.get(`/v1/content/album/${id}/resolved`);
    console.log(response.data);
    data.value = response.data;
  } catch (error) {
    console.error('Error fetching data:', error);
  }
};

const handleClickOnFavoriteIcon = () => {
  userStore.setAlbumIsLiked(data.value.album.id, !isAlbumLiked.value);
}

const handleClickOnPlayAlbum = () => {
  player.setResolvedAlbum(data.value);
}

const handleClickOnTrack = (trackId, discIndex, trackIndex) => {
  if (trackId != currentTrackId.value) {
    player.setResolvedAlbum(data.value, discIndex, trackIndex);
  }
}

watch(data,
  (newData) => {
    if (newData) {
      coverUrls.value = chooseAlbumCoverImageUrl(newData.album);
    }
  },
  { immediate: true }
);

watch(() => props.albumId, (newId) => {
  fetchData(newId);
});

watch([() => userStore.likedAlbumIds, data],
  ([likedAlbums, albumData], [oldLikedAlbums, oldAlbumData]) => {
    console.log("watch liked albums and album data, new stuff incoming: " + likedAlbums + " " + albumData);
    if (likedAlbums && albumData) {
      isAlbumLiked.value = likedAlbums.includes(albumData.album.id);
      console.log("isAlbumLiked: " + isAlbumLiked.value);
      console.log("likedAlbums: " + likedAlbums);
    }
  },
  { immediate: true }
);

onMounted(() => {
  fetchData(props.albumId);
  userStore.triggerAlbumsLoad();
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

.albumInfoColum {
  margin: 0 16px;
  display: flex;
  flex-direction: column;
}

.albumName {
  flex: 1;
}

.playAlbumIcon {
  scale: 1.2;
  fill: var(--accent-color);
  cursor: pointer;
  transition: scale 0.3s ease;
}

.playAlbumIcon:hover {
  scale: 1.3;
  transition: scale 0.3s ease;
}

.commandsSection {
  display: flex;
  flex-direction: row;
  margin-top: 16px;
  margin-left: 8px;
  margin-right: 8px;
}

.commandsSection>div {
  margin-left: 16px;
}

.artistsContainer {
  width: 100%;
  display: flex;
  flex-direction: row;
  overflow-x: auto;
}

.trackRow {
  display: flex;
  flex-direction: row;
  padding: 8px 0;
  align-items: center;
}

.nonPlayingTrack:hover {
  background-color: var(--highlighted-panel-color);
  cursor: pointer;
}

.playingTrack {
  color: var(--accent-color);
}

.trackIndexSpan {
  width: 24px;
  padding-right: 12px;
  align-items: right;
  align-content: right;

  justify-content: right;
  justify-items: right;
  justify-self: right;
}

.trackNameSpan {
  flex: 1;
  size: 14px !important;
}

.trackArtistsSpan {
  flex: 1;
}

.trackDurationSpan {}
</style>
