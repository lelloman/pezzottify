<template>
  <div class="searchResultRow" :data-id="result" @click.stop="handleTrackClick(result)"
    @contextmenu.prevent="openContextMenu($event, result.id)">
    <MultiSourceImage :urls="[imageUrl]" alt="Image" class="searchResultImage scaleClickFeedback"
      @click.stop="handleImageClick" />
    <div class="column">
      <TrackName :track="result" class="trackName" :hoverAnimation="true" />
      <ClickableArtistsNames :artistsIdsNames="result.artists_ids_names" />
    </div>
    <h3 class="duration">{{ duration }}</h3>
    <PlayIcon class="searchResultPlayIcon scaleClickFeedback bigIcon" :data-id="result"
      @click.stop="handlePlayClick(result)" />
  </div>
  <TrackContextMenu ref="trackContextMenuRef" />
</template>

<script setup>
import '@/assets/search.css'
import { ref } from 'vue';
import { computedImageUrl, formatDuration } from '@/utils';
import { usePlayerStore } from '@/store/player';
import { useRouter } from 'vue-router';
import PlayIcon from '@/components/icons/PlayIcon.vue';
import ClickableArtistsNames from '@/components/common/ClickableArtistsNames.vue';
import MultiSourceImage from '@/components/common/MultiSourceImage.vue';
import TrackName from '../common/TrackName.vue';
import TrackContextMenu from '@/components/common/contextmenu/TrackContextMenu.vue';

const props = defineProps({
  result: {
    type: Object,
    required: true,
  }
});
const imageUrl = computedImageUrl(props.result.image_id);


const duration = formatDuration(props.result.duration);

const playerStore = usePlayerStore();
const router = useRouter();

const trackContextMenuRef = ref(null);
const openContextMenu = (event, trackId) => {
  trackContextMenuRef.value.openMenu(event, trackId, 0);
}

const handleTrackClick = (event) => {
  console.log("trackClick");
  console.log(event);
  router.push("/track/" + event.id);
}

const handlePlayClick = (event) => {
  console.log("play click");
  console.log(event);
  playerStore.setTrack(event);
  playerStore.setIsPlaying(true);
}

const handleImageClick = () => {
  router.push("/album/" + props.result.album_id);
}

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
</style>
