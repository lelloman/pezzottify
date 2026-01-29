<template>
  <div
    :class="computeRowClasses"
    :data-id="result"
    @click.stop="handleTrackClick(result)"
    @contextmenu.prevent="openContextMenu($event, result.id)"
  >
    <MultiSourceImage
      :urls="[imageUrl]"
      alt="Image"
      class="searchResultImage scaleClickFeedback"
      @click.stop="handleImageClick"
    />
    <div class="column">
      <TrackName :track="result" class="trackName" :hoverAnimation="true" />
      <ClickableArtistsNames :artistsIdsNames="result.artists_ids_names" />
    </div>
    <div v-if="isTrackFetchError" class="track-fetch-error-icon" title="Download failed">⚠</div>
    <h3 class="duration">{{ duration }}</h3>
    <PlayIcon
      v-if="isTrackAvailable"
      class="searchResultPlayIcon scaleClickFeedback bigIcon"
      :data-id="result"
      @click.stop="handlePlayClick(result)"
    />
  </div>
  <TrackContextMenu ref="trackContextMenuRef" />
</template>

<script setup>
import "@/assets/search.css";
import { ref, computed } from "vue";
import { computedImageUrl, formatDuration } from "@/utils";
import { usePlaybackStore } from "@/store/playback";
import { useRouter } from "vue-router";
import PlayIcon from "@/components/icons/PlayIcon.vue";
import ClickableArtistsNames from "@/components/common/ClickableArtistsNames.vue";
import MultiSourceImage from "@/components/common/MultiSourceImage.vue";
import TrackName from "../common/TrackName.vue";
import TrackContextMenu from "@/components/common/contextmenu/TrackContextMenu.vue";

const props = defineProps({
  result: {
    type: Object,
    required: true,
  },
});
// Image endpoint takes album ID (tracks use their album's image)
const imageUrl = computedImageUrl(props.result.album_id);

const duration = formatDuration(props.result.duration);

const isTrackAvailable = computed(() => {
  return !props.result.availability || props.result.availability === "available";
});

const isTrackFetching = computed(() => {
  return props.result.availability === "fetching";
});

const isTrackFetchError = computed(() => {
  return props.result.availability === "fetch_error";
});

const computeRowClasses = computed(() => {
  return {
    searchResultRow: true,
    trackUnavailable: !isTrackAvailable.value && !isTrackFetching.value,
    trackFetching: isTrackFetching.value,
  };
});

const playbackStore = usePlaybackStore();
const router = useRouter();

const trackContextMenuRef = ref(null);
const openContextMenu = (event, trackId) => {
  trackContextMenuRef.value.openMenu(event, trackId, 0);
};

const handleTrackClick = (event) => {
  console.log("trackClick");
  console.log(event);
  router.push("/track/" + event.id);
};

const handlePlayClick = (event) => {
  if (!isTrackAvailable.value) {
    return;
  }
  console.log("play click");
  console.log(event);
  playbackStore.setTrack(event);
};

const handleImageClick = () => {
  router.push("/album/" + props.result.album_id);
};
</script>

<style scoped>
.column {
  display: flex;
  flex-direction: column;
  flex: 1;
  width: 0;
  margin-right: 8px;
}

.title {
  margin: 0;
  font-size: 16px;
  font-weight: bold;
}

.subtitle {
  margin: 0;
  font-size: 14px;
  color: #666;
}

.duration {
  text-align: center;
  vertical-align: middle;
  height: 100%;
}

.trackName {
  flex: 1;
  width: 100%;
}

/* Track availability states */
.trackUnavailable {
  opacity: 0.4;
  cursor: not-allowed;
}

.trackUnavailable:hover {
  background-color: transparent;
}

.trackFetching {
  animation: trackFetchingPulse 1.5s ease-in-out infinite;
}

@keyframes trackFetchingPulse {
  0%, 100% {
    opacity: 1;
  }
  50% {
    opacity: 0.4;
  }
}

.track-fetch-error-icon {
  color: var(--warning);
  font-size: 16px;
  margin-right: 8px;
}
</style>
