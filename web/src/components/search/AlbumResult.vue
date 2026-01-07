<template>
  <div class="searchResultRow" :data-id="result" @click="handleClick(result)">
    <MultiSourceImage
      :urls="[imageUrl]"
      alt="Image"
      class="searchResultImage"
    />
    <div class="column">
      <h3 class="title">{{ result.name }}</h3>
      <ClickableArtistsNames
        class="subtitle"
        :prefix="result.year + ' - '"
        :artistsIdsNames="result.artists_ids_names"
      />
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
import { usePlayerStore } from "@/store/player";
import { computedImageUrl } from "@/utils.js";
import { useRouter } from "vue-router";
import PlayIcon from "@/components/icons/PlayIcon.vue";
import ClickableArtistsNames from "@/components/common/ClickableArtistsNames.vue";
import MultiSourceImage from "../common/MultiSourceImage.vue";

const playerStore = usePlayerStore();

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
  playerStore.setAlbumId(event.id);
  playerStore.setIsPlaying(true);
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
</style>
