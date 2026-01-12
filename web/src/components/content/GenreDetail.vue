<template>
  <div class="genreDetailPage">
    <div class="genreHeader">
      <h1 class="genreName">{{ decodedGenreName }}</h1>
      <div class="headerInfo">
        <span v-if="genreData" class="trackCount">{{ formatTrackCount(genreData.total) }}</span>
      </div>
    </div>

    <div class="actionsRow">
      <button class="shuffleButton" @click="handleShufflePlay" :disabled="isLoadingRadio">
        <PlayIcon class="buttonIcon" />
        <span>{{ isLoadingRadio ? "Loading..." : "Shuffle Play" }}</span>
      </button>
    </div>

    <!-- Loading State -->
    <div v-if="isLoading" class="loadingState">Loading tracks...</div>

    <!-- Track List -->
    <div v-else-if="genreData && genreData.track_ids.length > 0" class="tracksSection">
      <div
        v-for="(trackId, trackIndex) in genreData.track_ids"
        :key="trackId"
        class="track"
      >
        <LoadTrackListItem
          :contextId="genreName"
          :trackId="trackId"
          :trackNumber="trackIndex + 1 + currentOffset"
          @track-clicked="handleTrackSelection"
        />
      </div>

      <!-- Load More Button -->
      <button
        v-if="genreData.has_more"
        class="loadMoreButton"
        @click="loadMore"
        :disabled="isLoadingMore"
      >
        {{ isLoadingMore ? "Loading..." : "Load More" }}
      </button>
    </div>

    <!-- Empty State -->
    <div v-else class="emptyState">
      <p>No tracks found for this genre</p>
    </div>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, watch } from "vue";
import { useRemoteStore } from "@/store/remote";
import { usePlayerStore } from "@/store/player";
import LoadTrackListItem from "@/components/common/LoadTrackListItem.vue";
import PlayIcon from "@/components/icons/PlayIcon.vue";

const props = defineProps({
  genreName: {
    type: String,
    required: true,
  },
});

const remoteStore = useRemoteStore();
const player = usePlayerStore();

const genreData = ref(null);
const isLoading = ref(true);
const isLoadingMore = ref(false);
const isLoadingRadio = ref(false);
const currentOffset = ref(0);
const TRACKS_PER_PAGE = 50;

const decodedGenreName = computed(() => decodeURIComponent(props.genreName));

const formatTrackCount = (count) => {
  if (count === 1) return "1 track";
  return `${count.toLocaleString()} tracks`;
};

const loadGenreTracks = async () => {
  isLoading.value = true;
  currentOffset.value = 0;
  genreData.value = await remoteStore.fetchGenreTracks(
    decodedGenreName.value,
    TRACKS_PER_PAGE,
    0
  );
  isLoading.value = false;
};

const loadMore = async () => {
  if (!genreData.value?.has_more || isLoadingMore.value) return;

  isLoadingMore.value = true;
  const newOffset = currentOffset.value + TRACKS_PER_PAGE;
  const moreData = await remoteStore.fetchGenreTracks(
    decodedGenreName.value,
    TRACKS_PER_PAGE,
    newOffset
  );

  if (moreData) {
    genreData.value = {
      ...genreData.value,
      track_ids: [...genreData.value.track_ids, ...moreData.track_ids],
      has_more: moreData.has_more,
    };
    currentOffset.value = newOffset;
  }
  isLoadingMore.value = false;
};

const handleTrackSelection = (track) => {
  // Create a pseudo-playlist from current tracks
  const playlist = {
    name: `${decodedGenreName.value} Radio`,
    tracks: genreData.value.track_ids,
  };
  player.setUserPlaylist(playlist);
  // Find the track's index in the loaded tracks
  const trackIndex = genreData.value.track_ids.indexOf(track.id);
  if (trackIndex >= 0) {
    player.loadTrack(trackIndex);
  }
};

const handleShufflePlay = async () => {
  isLoadingRadio.value = true;
  const radioTracks = await remoteStore.fetchGenreRadio(decodedGenreName.value, 50);
  isLoadingRadio.value = false;

  if (radioTracks && radioTracks.length > 0) {
    const playlist = {
      name: `${decodedGenreName.value} Radio`,
      tracks: radioTracks,
    };
    player.setUserPlaylist(playlist);
  }
};

onMounted(() => {
  loadGenreTracks();
});

// Reload when genre changes
watch(
  () => props.genreName,
  () => {
    loadGenreTracks();
  }
);
</script>

<style scoped>
.genreDetailPage {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-4);
}

.genreHeader {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-1);
}

.genreName {
  font-size: var(--text-2xl);
  font-weight: var(--font-bold);
  color: var(--text-base);
  margin: 0;
  text-transform: capitalize;
}

.headerInfo {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
}

.trackCount {
  font-size: var(--text-sm);
  color: var(--text-subdued);
}

.actionsRow {
  display: flex;
  gap: var(--spacing-3);
}

.shuffleButton {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  padding: var(--spacing-2) var(--spacing-4);
  background-color: var(--essential-bright-accent);
  color: var(--text-base);
  border: none;
  border-radius: var(--radius-full);
  font-size: var(--text-sm);
  font-weight: var(--font-semibold);
  cursor: pointer;
  transition: opacity var(--transition-fast);
}

.shuffleButton:hover {
  opacity: 0.9;
}

.shuffleButton:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.buttonIcon {
  width: 16px;
  height: 16px;
}

.tracksSection {
  display: flex;
  flex-direction: column;
}

.track {
  border-bottom: 1px solid var(--essential-subdued);
}

.track:last-child {
  border-bottom: none;
}

.loadMoreButton {
  margin-top: var(--spacing-4);
  padding: var(--spacing-3) var(--spacing-4);
  background-color: transparent;
  color: var(--text-base);
  border: 1px solid var(--essential-subdued);
  border-radius: var(--radius-md);
  font-size: var(--text-sm);
  cursor: pointer;
  transition: background-color var(--transition-fast);
}

.loadMoreButton:hover {
  background-color: var(--bg-elevated-highlight);
}

.loadMoreButton:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.loadingState,
.emptyState {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: var(--spacing-8);
  color: var(--text-subdued);
  text-align: center;
}

.emptyState p {
  margin: 0;
}
</style>
