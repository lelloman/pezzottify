<template>
  <div class="trackWrapper">
    <div v-if="loading" class="track-loading">Loading...</div>
    <div v-else-if="error" class="track-error">Error: {{ error }}</div>
    <div v-else-if="track" @click="handleTrackClick" :class="computeTrackRowClasses(props.trackId)">
      <div class="trackIndexSpan">
        <p>{{ trackNumber }} </p>
      </div>
      <TrackName :track="track" class="trackNameSpan" :hoverAnimation="true" />
      <div class="trackArtistsSpan">
        <LoadClickableArtistsNames :artistsIds="track.artists_ids" />
      </div>
      <div class="track-duration">{{ formatDuration(track.duration) }}</div>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted, watch } from 'vue';
import { usePlayerStore } from '@/store/player';
import { formatDuration } from '@/utils';
import TrackName from '@/components/common/TrackName.vue';
import LoadClickableArtistsNames from '@/components/common/LoadClickableArtistsName.vue';
import { useStaticsStore } from '@/store/statics';

const player = usePlayerStore();
const staticsStore = useStaticsStore();

const props = defineProps({
  trackId: {
    type: String,
    required: true,
  },
  contextId: {
    type: String,
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
const track = ref(null);

let trackDataUnWatcher = null;

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
  if (trackDataUnWatcher) {
    trackDataUnWatcher();
    trackDataUnWatcher = null;
  }
  loading.value = true;
  error.value = null;
  track.value = null;
  trackDataUnWatcher = watch(staticsStore.getTrack(props.trackId),
    (newTrack) => {
      console.log('LoadTrackListItem New track:', newTrack);
      if (newTrack && newTrack.item) {
        loading.value = false;

        if (newTrack.error) {
          error.value = newTrack.error;
        }
        if (typeof newTrack.item === 'object') {
          track.value = newTrack.item;
        }
      }
    },
    { immediate: true }
  );
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
  loadTrackData();
});

// Watch for changes to trackId and reload if necessary
watch(() => props.trackId, (newId, oldId) => {
  if (newId != oldId) {
    //loadTrackData();
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
  margin-right: 16px;
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
