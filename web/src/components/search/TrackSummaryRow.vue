<template>
  <div class="trackRow" @click="handleClick">
    <MultiSourceImage :urls="[imageUrl]" alt="Track" class="trackImage" />
    <div class="trackInfo">
      <span class="trackName">{{ track.name }}</span>
      <span class="trackArtists">{{ artistNames }}</span>
    </div>
    <span class="trackDuration">{{ formattedDuration }}</span>
    <PlayIcon class="playIcon scaleClickFeedback" @click.stop="handlePlayClick" />
  </div>
</template>

<script setup>
import { computed } from "vue";
import { useRouter } from "vue-router";
import { usePlayerStore } from "@/store/player";
import { computedImageUrl, formatDuration } from "@/utils";
import MultiSourceImage from "@/components/common/MultiSourceImage.vue";
import PlayIcon from "@/components/icons/PlayIcon.vue";

const props = defineProps({
  track: {
    type: Object,
    required: true,
  },
});

const router = useRouter();
const playerStore = usePlayerStore();

// Image endpoint takes album ID (tracks use their album's image)
const imageUrl = computedImageUrl(props.track.album_id);

const artistNames = computed(() => {
  return props.track.artist_names ? props.track.artist_names.join(", ") : "";
});

const formattedDuration = computed(() => {
  // duration_ms is in milliseconds
  const seconds = Math.floor(props.track.duration_ms / 1000);
  return formatDuration(seconds);
});

const handleClick = () => {
  router.push("/album/" + props.track.album_id);
};

const handlePlayClick = () => {
  // Create a minimal track object for the player
  const trackForPlayer = {
    id: props.track.id,
    name: props.track.name,
    duration: Math.floor(props.track.duration_ms / 1000),
    album_id: props.track.album_id,
    artists_ids_names: props.track.artist_names
      ? props.track.artist_names.map((name) => ["", name])
      : [],
  };
  playerStore.setTrack(trackForPlayer);
  playerStore.setIsPlaying(true);
};
</script>

<style scoped>
.trackRow {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px;
  border-radius: var(--radius-md);
  cursor: pointer;
  transition: background-color 0.2s ease;
}

.trackRow:hover {
  background-color: var(--bg-elevated-highlight);
}

.trackImage {
  width: 48px;
  height: 48px;
  border-radius: var(--radius-sm);
  object-fit: cover;
}

.trackInfo {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.trackName {
  font-weight: var(--font-medium);
  color: var(--text-base);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.trackArtists {
  font-size: var(--text-sm);
  color: var(--text-subdued);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.trackDuration {
  color: var(--text-subdued);
  font-size: var(--text-sm);
  min-width: 40px;
  text-align: right;
}

.playIcon {
  width: 32px;
  height: 32px;
  opacity: 0;
  transition: opacity 0.2s ease;
}

.trackRow:hover .playIcon {
  opacity: 1;
}
</style>
