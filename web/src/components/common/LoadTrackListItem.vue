<template>
  <div class="trackWrapper">
    <div v-if="loading" class="track-loading">Loading...</div>
    <div v-else-if="error" class="track-error">Error: {{ error }}</div>
    <div v-else-if="track" @click="handleTrackClick" :class="computeTrackRowClasses(props.trackId)">
      <div class="trackIndexSpan">
        <p>{{ trackNumber }} </p>
      </div>
      <TrackName :track="track" class="trackNameSpan" />
      <div class="trackArtistsSpan">
        <LoadClickableArtistsNames :artistsIds="track.artists_ids" />
      </div>
      <div class="track-duration">{{ formatDuration(track.duration) }}</div>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted, computed, watch } from 'vue';
import { usePlayerStore } from '@/store/player';
import { useRemoteStore } from '@/store/remote';
import { formatDuration } from '@/utils';
import TrackName from '@/components/common/TrackName.vue';
import LoadClickableArtistsNames from '@/components/common/LoadClickableArtistsName.vue';

const player = usePlayerStore();
const remoteStore = useRemoteStore();

const props = defineProps({
  trackId: {
    type: String,
    required: true,
  },
  resolvedTrack: {
    type: Object,
    default: null
  },
  trackNumber: {
    type: Number,
    default: 0
  }
});

const emit = defineEmits(['track-clicked']);

const currentTrackId = ref(null);

const loading = ref(false);
const error = ref(null);
const resolvedData = ref(null);

const track = computed(() => {
  // Use resolved track if provided, otherwise use fetched data
  if (props.resolvedTrack) {
    return props.resolvedTrack;
  }
  return resolvedData.value;
});

watch(() => player.currentTrack,
  (newTrack) => {
    if (newTrack) {
      console.log("CurrentTrackId: " + newTrack.id);
      currentTrackId.value = newTrack.id;
    }
  },
  { immediate: true }
);


const loadTrackData = async () => {
  // If we already have resolved track data or no trackId, don't fetch
  if (props.resolvedTrack || !props.trackId) return;

  loading.value = true;
  error.value = null;

  try {
    resolvedData.value = await remoteStore.fetchTrackData(props.trackId);
    if (!resolvedData.value) {
      error.value = 'Failed to load track data';
    }
  } catch (err) {
    console.error('Failed to load track data:', err);
    error.value = 'Failed to load track data';
  } finally {
    loading.value = false;
  }
};

const computeTrackRowClasses = (trackId) => {
  const isCurrentTrack = trackId == currentTrackId.value;
  return {
    trackRow: true,
    nonPlayingTrack: !isCurrentTrack,
    playingTrack: isCurrentTrack,
  };
}

const handleTrackClick = () => {
  if (track.value) {
    emit('track-clicked', track.value);
  }
};

// Load track data if needed when component mounts
onMounted(() => {
  const shouldResolve = !props.resolvedTrack
  console.log("LoadTrackListItem onMounted shouldResolve: ", shouldResolve + " track: " + props.resolvedTrack + " trackId: " + props.trackId);
  console.log(props.resolvedTrack);
  if (shouldResolve) {
    loadTrackData();
  }
});

// Watch for changes to trackId and reload if necessary
watch(() => props.trackId, (newId, oldId) => {
  if (newId !== oldId && !props.resolvedTrack) {
    loadTrackData();
  }
});

// Expose track data for parent components
defineExpose({
  track
});
</script>

<style scoped>
.trackWrapper {
  width: 100%;
}

.trackRow {
  display: flex;
  flex-direction: row;
  padding: 8px 0;
  align-items: center;
}

.nonPlayingTrack:hover {
  background-color: var(--highlighted-panel-color);
  cursor: pointer;
}

.playingTrack {
  color: var(--accent-color);
}

.trackIndexSpan {
  width: 24px;
  padding-right: 12px;
  align-items: right;
  align-content: right;

  justify-content: right;
  justify-items: right;
  justify-self: right;
}

.trackNameSpan {
  width: 0;
  flex: 1;
  size: 14px !important;
  padding-right: 8px;
}

.trackArtistsSpan {
  flex: 1;
  width: 0;
  padding-right: 8px;
}

.track-number {
  width: 30px;
  text-align: right;
  margin-right: 16px;
  color: #b3b3b3;
  font-size: 0.9rem;
}

.playing-icon {
  color: #1ed760;
}

.trackNameSpan {
  flex: 1;
  size: 14px !important;
}
</style>
