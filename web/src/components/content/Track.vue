<template>
  <div v-if="track">
    <div class="topSection">
      <MultiSourceImage class="coverImage" :urls="coverUrls" />
      <div class="trackInfoColum">
        <h1 class="trackName">{{ track.name }}</h1>
        <h3 v-if="album">
          From:
          <span class="albumName" @click.stop="handleClickOnAlbumName"
            >{{ album.name }} - {{ getYearFromTimestamp(album.date) }}</span
          >
        </h3>
        <p>Duration: {{ formatDuration(track.duration) }}</p>
        <p v-if="track.is_explicit">Explicit!</p>
        <p v-if="track.has_lyrics && track.language_of_performance.length">
          Language: {{ track.language_of_performance.join(", ") }}
        </p>
      </div>
    </div>
    <div class="commandsSection">
      <PlayIcon
        class="playTrackIcon scaleClickFeedback bigIcon"
        @click.stop="handleClickOnPlayTrack"
      />
    </div>
    <div class="artistsContainer">
      <LoadArtistListItem
        v-for="artistId in track.artists_ids"
        :key="artistId"
        :artistId="artistId"
      />
    </div>
  </div>
  <div v-else>
    <p>Loading {{ trackId }}...</p>
  </div>
</template>

<script setup>
import { ref, watch, onMounted } from "vue";
import MultiSourceImage from "@/components/common/MultiSourceImage.vue";
import PlayIcon from "../icons/PlayIcon.vue";
import { usePlayerStore } from "@/store/player";
import { useRemoteStore } from "@/store/remote";
import {
  chooseAlbumCoverImageUrl,
  formatDuration,
  getYearFromTimestamp,
} from "@/utils";
import { useRouter } from "vue-router";
import LoadArtistListItem from "../common/LoadArtistListItem.vue";
import { useStaticsStore } from "@/store/statics";

const props = defineProps({
  trackId: {
    type: String,
    required: true,
  },
});

const track = ref(null);
const album = ref(null);
const coverUrls = ref([]);

const router = useRouter();
const player = usePlayerStore();
const staticsStore = useStaticsStore();
const remoteStore = useRemoteStore();

let trackDataUnwatcher = null;
let albumDataUnwatcher = null;

const handleClickOnPlayTrack = () => {
  if (track.value) {
    player.setTrack({
      id: track.value.id,
      name: track.value.name,
      artists: track.value.artists_ids,
      duration: track.value.duration,
      album_id: track.value.album_id,
    });
  }
};

const handleClickOnAlbumName = () => {
  router.push("/album/" + album.value.id);
};

const fetchTrack = async (id) => {
  if (trackDataUnwatcher) {
    trackDataUnwatcher();
    trackDataUnwatcher = null;
  }
  if (albumDataUnwatcher) {
    albumDataUnwatcher();
    albumDataUnwatcher = null;
  }
  track.value = null;
  coverUrls.value = [];

  if (!id) return;

  trackDataUnwatcher = watch(
    staticsStore.getTrack(id),
    (newData) => {
      if (newData && newData.item && typeof newData.item === "object") {
        track.value = newData.item;
        albumDataUnwatcher = watch(
          staticsStore.getAlbum(newData.item.album_id),
          (newAlbumData) => {
            if (
              newAlbumData &&
              newAlbumData.item &&
              typeof newAlbumData.item === "object"
            ) {
              album.value = newAlbumData.item;
              coverUrls.value = chooseAlbumCoverImageUrl(newAlbumData.item);
            }
          },
          { immediate: true },
        );
      }
    },
    { immediate: true },
  );
};
watch(
  () => props.trackId,
  (newId) => {
    fetchTrack(newId);
    if (newId) {
      remoteStore.recordImpression("track", newId);
    }
  },
);

onMounted(() => {
  fetchTrack(props.trackId);
  remoteStore.recordImpression("track", props.trackId);
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
  object-fit: contain;
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
