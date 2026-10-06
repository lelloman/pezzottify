<template>
  <DetailPage :title="decodedGenreName" kind="Genre">
    <template #meta
      ><span v-if="genreData">{{
        formatTrackCount(genreData.total)
      }}</span></template
    >
    <template #actions
      ><DetailActions
        :playLabel="isLoadingRadio ? 'Loading radio' : 'Shuffle play'"
        :disabled="isLoadingRadio"
        @play="handleShufflePlay"
    /></template>
    <!-- Loading State -->
    <div v-if="isLoading" class="loadingState">Loading tracks...</div>

    <!-- Track List -->
    <div
      v-else-if="genreData && genreData.track_ids.length > 0"
      class="tracksSection"
    >
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
  </DetailPage>
</template>

<script setup>
import { ref, computed, onMounted, watch } from "vue";
import { useRemoteStore } from "@/store/remote";
import { usePlaybackStore } from "@/store/playback";
import LoadTrackListItem from "@/components/common/LoadTrackListItem.vue";
import DetailPage from "@/components/common/DetailPage.vue";
import DetailActions from "@/components/common/DetailActions.vue";

const props = defineProps({
  genreName: {
    type: String,
    required: true,
  },
});

const remoteStore = useRemoteStore();
const playback = usePlaybackStore();

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
    0,
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
    newOffset,
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
  playback.setUserPlaylist(playlist);
  // Find the track's index in the loaded tracks
  const trackIndex = genreData.value.track_ids.indexOf(track.id);
  if (trackIndex >= 0) {
    playback.loadTrackIndex(trackIndex);
  }
};

const handleShufflePlay = async () => {
  isLoadingRadio.value = true;
  await playback.createGenreRadio(decodedGenreName.value, 50);
  isLoadingRadio.value = false;
};

onMounted(() => {
  loadGenreTracks();
});

// Reload when genre changes
watch(
  () => props.genreName,
  () => {
    loadGenreTracks();
  },
);
</script>

<style scoped>
.genreName {
  font-size: var(--text-2xl);
  font-weight: var(--font-bold);
  color: var(--text-base);
  margin: 0;
  text-transform: capitalize;
}

.tracksSection {
  display: flex;
  flex-direction: column;
}

.track {
  border-bottom: none;
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
