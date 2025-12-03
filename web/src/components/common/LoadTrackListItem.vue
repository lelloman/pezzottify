<template>
  <div class="trackWrapper">
    <div v-if="loading" class="track-loading">Loading...</div>
    <div v-else-if="error" class="track-error">Error: {{ error }}</div>
    <div v-else-if="track" @click="handleTrackClick" :class="computeTrackRowClasses">
      <div class="track-item-content">
        <div class="trackIndexSpan">
          <p>{{ trackNumber }} </p>
        </div>
        <MultiSourceImage v-if="track.image_urls" class="trackImage scaleClickFeedback" :urls="track.image_urls"
          @click.stop="$emit('track-image-clicked', track)" />
        <TrackName :track="track" class="trackNameSpan" :hoverAnimation="true" />
        <div class="trackArtistsSpan">
          <LoadClickableArtistsNames v-if="track.artists_ids" :artistsIds="track.artists_ids" />
        </div>
        <div class="track-duration">{{ formatDuration(track.duration) }}</div>
      </div>
    </div>
  </div>
</template>

<script setup>
import { ref, onMounted, computed, watch } from 'vue';
import { formatDuration } from '@/utils';
import TrackName from '@/components/common/TrackName.vue';
import LoadClickableArtistsNames from '@/components/common/LoadClickableArtistsNames.vue';
import { useStaticsStore } from '@/store/statics';
import MultiSourceImage from '@/components/common/MultiSourceImage.vue';

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
  },
  isCurrentlyPlaying: {
    type: Boolean,
    default: false
  }
});

const emit = defineEmits(['track-clicked', 'track-image-clicked']);

const loading = ref(false);
const error = ref(null);
const track = ref(null);

let trackDataUnWatcher = null;

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

const computeTrackRowClasses = computed(() => {
  return {
    trackRow: true,
    nonPlayingTrack: !props.isCurrentlyPlaying,
    playingTrack: props.isCurrentlyPlaying,
  };
});

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
  color: var(--text-base);
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
  width: 36px;
  padding-right: 12px;
  text-align: right;
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

.track-item-content {
  display: flex;
  flex-direction: row;
  width: 100%;
  align-items: center;
  padding: 4px 0;
}

.trackImage {
  width: 40px;
  height: 40px;
  margin-right: 8px;
}
</style>
