<template>
  <div v-if="album">
    <div class="topSection">
      <MultiSourceImage class="coverImage" :urls="coverUrls" />
      <div class="albumInfoColum">
        <h1 class="albumName"> {{ album.name }}</h1>
      </div>
    </div>
    <div class="commandsSection">
      <PlayIcon class="playAlbumIcon scaleClickFeedback bigIcon" @click.stop="handleClickOnPlayAlbum" />
      <ToggableFavoriteIcon :toggled="isAlbumLiked" :clickCallback="handleClickOnFavoriteIcon" />
    </div>
    <div class="artistsContainer">
      <LoadArtistListItem v-for="artistId in album.artists_ids" :key="artistId" :artistId="artistId" />
    </div>
    <div class="tracksContainer">
      <div v-for="(disc, discIndex) in album.discs" :key="disc" class="discContainer">
        <h1 v-if="album.discs.length > 1">Disc {{ discIndex + 1 }}<span v-if="disc.name">- {{ disc.name }}</span>
        </h1>
        <div v-for="(trackId, trackIndex) in disc.tracks" :key="trackId" class="track"
          @contextmenu.prevent="openTrackContextMenu($event, data.tracks[trackId], trackIndex)">
          <LoadTrackListItem :contextId="albumId" :trackId="trackId" :trackNumber="trackIndex + 1"
            @track-clicked="handleClickOnTrack(trackId)" />
        </div>
      </div>
    </div>
    <TrackContextMenu ref="trackContextMenuRef" />
  </div>
  <div v-else>
    <p>Loading {{ albumId }}...</p>
  </div>
</template>

<script setup>
import { ref, watch, onMounted } from 'vue';
import { chooseAlbumCoverImageUrl } from '@/utils';
import MultiSourceImage from '@/components/common/MultiSourceImage.vue';
import PlayIcon from '@/components/icons/PlayIcon.vue';
import { usePlayerStore } from '@/store/player';
import { useUserStore } from '@/store/user';
import ToggableFavoriteIcon from '@/components/common/ToggableFavoriteIcon.vue';
import LoadArtistListItem from '@/components/common/LoadArtistListItem.vue';
import TrackContextMenu from '@/components/common/contextmenu/TrackContextMenu.vue';
import LoadTrackListItem from '../common/LoadTrackListItem.vue';
import { useStaticsStore } from '@/store/statics';

const props = defineProps({
  albumId: {
    type: String,
    required: true,
  }
});

const album = ref(null);
const coverUrls = ref(null);

const player = usePlayerStore();
const userStore = useUserStore();
const staticsStore = useStaticsStore();

const currentTrackId = ref(null);
const isAlbumLiked = ref(false);

let albumDataUnwatcher = null;

const trackContextMenuRef = ref(null);
const openTrackContextMenu = (event, track, index) => {
  trackContextMenuRef.value.openMenu(event, track, index);
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
  if (albumDataUnwatcher) {
    albumDataUnwatcher();
    albumDataUnwatcher = null;
  }
  if (!id) return;

  albumDataUnwatcher = watch(
    staticsStore.getAlbum(id),
    (newData) => {
      if (newData && newData.item && typeof newData.item === 'object') {
        coverUrls.value = chooseAlbumCoverImageUrl(newData.item);
        album.value = newData.item;
      }
    },
    { immediate: true });
};

const handleClickOnFavoriteIcon = () => {
  userStore.setAlbumIsLiked(props.albumId, !isAlbumLiked.value);
}

const handleClickOnPlayAlbum = () => {
  player.setAlbumId(props.albumId);
}

const handleClickOnTrack = (trackId) => {
  if (trackId != currentTrackId.value) {
    const discIndex = album.value.discs.findIndex((disc) => disc.tracks.includes(trackId));
    const trackIndex = album.value.discs[discIndex].tracks.indexOf(trackId);
    player.setAlbumId(props.albumId, discIndex, trackIndex);
  }
}

watch(album,
  (newAlbum) => {
    if (newAlbum) {
      coverUrls.value = chooseAlbumCoverImageUrl(newAlbum);
    }
  },
  { immediate: true }
);

watch(() => props.albumId, (newId) => {
  fetchData(newId);
});

watch(() => userStore.likedAlbumIds,
  (likedAlbums) => {
    console.log("watch liked albums and album data, new stuff incoming: " + likedAlbums);
    if (likedAlbums) {
      isAlbumLiked.value = likedAlbums.includes(props.albumId);
      console.log("isAlbumLiked: " + isAlbumLiked.value);
      console.log("likedAlbums: " + likedAlbums);
    }
  },
  { immediate: true }
);

onMounted(() => {
  fetchData(props.albumId);
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
  width: 64px;
  height: 64px;
  fill: var(--accent-color);
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
