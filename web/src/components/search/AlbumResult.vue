<template>
  <div class="searchResultRow" :data-id="result" @click="handleClick(result)">
    <MultiSourceImage
      :urls="[imageUrl]"
      alt="Image"
      class="searchResultImage"
      :class="{ 'image-unavailable': result.availability === 'missing' }"
    />
    <div class="column">
      <h3 class="title">{{ result.name }}</h3>
      <ClickableArtistsNames
        class="subtitle"
        :prefix="result.year + ' - '"
        :artistsIdsNames="result.artists_ids_names"
      />
    </div>
    <div class="availability-indicator" :class="result.availability">
      <span v-if="result.availability === 'missing'" class="missing-badge">Not available</span>
      <span v-else-if="result.availability === 'partial'" class="partial-badge">Partial</span>
      <span v-else class="complete-indicator"></span>
    </div>
    <PlayIcon
      class="searchResultPlayIcon scaleClickFeedback bigIcon"
      :data-id="result"
      @click.stop="handlePlayClick(result)"
    />
  </div>
</template>

<script setup>
import "@/assets/search.css";
import { usePlaybackStore } from "@/store/playback";
import { computedImageUrl } from "@/utils.js";
import { useRouter } from "vue-router";
import PlayIcon from "@/components/icons/PlayIcon.vue";
import ClickableArtistsNames from "@/components/common/ClickableArtistsNames.vue";
import MultiSourceImage from "../common/MultiSourceImage.vue";

const playbackStore = usePlaybackStore();

const props = defineProps({
  result: {
    type: Object,
    required: true,
  },
});

// Image endpoint now takes the album ID directly
const imageUrl = computedImageUrl(props.result.id);

const router = useRouter();

const handleClick = (event) => {
  console.log(event);
  router.push("/album/" + event.id);
};

const handlePlayClick = (event) => {
  console.log("play click");
  console.log(event);
  playbackStore.setAlbumId(event.id);
};
</script>

<style scoped>
.column {
  display: flex;
  flex-direction: column;
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

.image-unavailable {
  opacity: 0.5;
  filter: grayscale(100%);
}

.availability-indicator {
  margin-right: 12px;
  font-size: 12px;
  font-weight: 500;
}

.missing-badge {
  color: #f44336;
  background: rgba(244, 67, 54, 0.1);
  padding: 2px 8px;
  border-radius: 4px;
}

.partial-badge {
  color: #ff9800;
  background: rgba(255, 152, 0, 0.1);
  padding: 2px 8px;
  border-radius: 4px;
}

.complete-indicator {
  display: none;
}
</style>
