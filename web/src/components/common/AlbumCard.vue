<template>
  <div class=".albumWrapper">
    <div v-if="loading">Loading...</div>
    <div
      v-else-if="album"
      class="searchResultRow"
      :data-id="album.id"
      @click="handleClick(album.id)"
    >
      <MultiSourceImage
        :urls="chooseAlbumCoverImageUrl(album)"
        class="searchResultImage scaleClickFeedback"
        :class="{ 'image-unavailable': albumData?.album_availability === 'missing' }"
      />
      <div class="column">
        <h3 class="title">{{ album.name }}</h3>
        <LoadClickableArtistsNames
          v-if="showArtists && album.artists_ids"
          class="artistsNames"
          :artistsIds="album.artists_ids"
        />
      </div>
      <div v-if="albumData?.album_availability === 'partial'" class="availability-badge partial">
        Partial
      </div>
      <PlayIcon
        class="searchResultPlayIcon scaleClickFeedback bigIcon"
        :data-id="album.id"
        @click.stop="handlePlayClick(album.id)"
      />
    </div>
    <div v-else-if="error">Error. {{ error }}</div>
  </div>
</template>

<script setup>
import "@/assets/base.css";
import "@/assets/search.css";
import "@/assets/icons.css";
import { ref, watch, computed } from "vue";
import { useRouter } from "vue-router";
import { chooseAlbumCoverImageUrl } from "@/utils";
import MultiSourceImage from "./MultiSourceImage.vue";
import PlayIcon from "@/components/icons/PlayIcon.vue";
import { usePlayerStore } from "@/store/player";
import LoadClickableArtistsNames from "@/components/common/LoadClickableArtistsNames.vue";
import { useStaticsStore } from "@/store/statics";

const router = useRouter();
const staticsStore = useStaticsStore();
const playerStore = usePlayerStore();

const props = defineProps({
  albumId: {
    type: String,
    required: false,
  },
  album: {
    type: Object,
    required: false,
  },
  showArtists: {
    type: Boolean,
    required: false,
    default: false,
  },
});

const album = ref(props.album || null);
const artistsRefs = ref(null);
const loading = ref(!props.album);
const error = ref(null);

const albumData = computed(() => {
  return props.album || album.value;
});

// Only fetch if we don't have album data passed directly
if (props.albumId && !props.album) {
  watch(
    staticsStore.getAlbum(props.albumId),
    (newData) => {
      loading.value = newData && newData.loading;
      if (newData && newData.item && typeof newData.item === "object") {
        artistsRefs.value = newData.item.artists_ids.map((artistId) =>
          staticsStore.getArtist(artistId),
        );
        album.value = newData.item;
      }
    },
    { immediate: true },
  );
} else if (props.album) {
  // Use provided album data directly
  album.value = props.album;
  loading.value = false;
}

const handlePlayClick = (event) => {
  playerStore.setAlbumId(event);
  playerStore.setIsPlaying(true);
};

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
  color: #ffffff !important;
}

.column {
  flex: 1;
  display: flex;
  flex-direction: column;
}

.image-unavailable {
  opacity: 0.5;
  filter: grayscale(100%);
}

.availability-badge {
  font-size: 11px;
  font-weight: 500;
  padding: 2px 6px;
  border-radius: 4px;
  margin-right: 8px;
}

.availability-badge.partial {
  color: #ff9800;
  background: rgba(255, 152, 0, 0.15);
}
</style>
