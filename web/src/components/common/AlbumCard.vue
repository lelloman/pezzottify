<template>
  <div class="albumWrapper">
    <div v-if="loading" class="albumState">Loading</div>
    <div
      v-else-if="album"
      class="searchResultRow albumListRow"
      :data-id="album.id"
      @click="handleClick(album.id)"
    >
      <MultiSourceImage
        :urls="chooseAlbumCoverImageUrl(album)"
        class="searchResultImage scaleClickFeedback"
        :class="{
          'image-unavailable': albumData?.album_availability === 'missing',
        }"
      />
      <div class="column">
        <h3 class="title">{{ album.name }}</h3>
        <LoadClickableArtistsNames
          v-if="showArtists && album.artists_ids"
          class="artistsNames"
          :artistsIds="album.artists_ids"
        />
      </div>
      <div
        v-if="albumData?.album_availability === 'partial'"
        class="availability-badge partial"
      >
        Partial
      </div>
      <PlayIcon
        class="searchResultPlayIcon scaleClickFeedback bigIcon"
        :data-id="album.id"
        @click.stop="handlePlayClick(album.id)"
      />
    </div>
    <div v-else-if="error" class="albumState errorState">
      Error. {{ error }}
    </div>
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
import { usePlaybackStore } from "@/store/playback";
import LoadClickableArtistsNames from "@/components/common/LoadClickableArtistsNames.vue";
import { useStaticsStore } from "@/store/statics";

const router = useRouter();
const staticsStore = useStaticsStore();
const playbackStore = usePlaybackStore();

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
  playbackStore.setAlbumId(event);
};

const handleClick = (albumId) => {
  router.push("/album/" + albumId);
};
</script>

<style scoped>
.albumWrapper {
  min-width: 0;
}

.albumState {
  display: flex;
  align-items: center;
  min-height: 64px;
  padding: 10px 12px;
  border-radius: 8px;
  color: var(--text-subdued);
  font-size: 0.82rem;
  font-weight: 700;
}

.errorState {
  color: #ffb4a8;
}

.relatedAlbumWrapper {
  max-width: 400px;
}

.albumListRow {
  padding-right: 42px;
}

.searchResultRoundImage {
  width: 80px;
  height: 80px;
  border-radius: 40px;
  margin-right: 10px;
}

.title {
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  margin: 0;
  font-size: 0.9rem;
  font-weight: 850;
  line-height: 1.18;
  color: #ffffff !important;
}

.column {
  flex: 1;
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 3px;
}

.artistsNames {
  display: block;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text-subdued);
  font-size: 0.76rem;
  font-weight: 620;
}

.image-unavailable {
  opacity: 0.5;
  filter: grayscale(100%);
}

.availability-badge {
  margin-right: 4px;
  border-radius: 4px;
  padding: 2px 6px;
  font-size: 0.68rem;
  font-weight: 800;
}

.availability-badge.partial {
  color: #ffb45f;
  background: rgba(255, 152, 0, 0.15);
}

.searchResultPlayIcon {
  top: 50%;
  right: 8px;
  bottom: auto;
  width: 30px;
  height: 30px;
  transform: translateY(-50%);
}
</style>
