<template>
  <div v-if="panelVisible" class="panel containero">
    <div class="header">
      <ChevronLeft :class="computePreviousPlaylistButtonClasses" @click.stop="seekPlaybackHistory(-1)" />
      <div class="currentlyPlaylistHeader">
        <SlidingText class="currePlaylistHeader" :hoverAnimation="true">
          <span class="currentlyPlayingText"> {{ playingContext.text }}</span>
        </SlidingText>
      </div>
      <ChevronRight :class="computeNextPlaylistButtonClasses" @click.stop="seekPlaybackHistory(1)" />
    </div>
    <div class="trackRowsContainer">
      <VirtualList v-model="tracksVModel" data-key="listItemId" @drop="handleDrop">
        <template v-slot:item="{ record, index, dataKey }">
          <div :class="{ currentlyPlayingRow: index == currentIndex, trackRow: true }"
            @contextmenu.prevent="openContextMenu($event, record.id, index)">
            <LoadTrackListItem :trackId="record.id" :trackNumber="index + 1" @track-clicked="handleClick(index)" :isCurrentlyPlaying="index == currentIndex" />
          </div>
        </template>
      </VirtualList>
    </div>

    <TrackContextMenu ref="trackContextMenuRef" :canRemoveFromQueue="true" />
  </div>
</template>
<script setup>
import '@/assets/main.css';
import { watch, ref, computed } from 'vue';
import { usePlayerStore } from '@/store/player';
import TrackContextMenu from '@/components/common/contextmenu/TrackContextMenu.vue';
import ChevronLeft from '@/components/icons/ChevronLeft.vue';
import ChevronRight from '@/components/icons/ChevronRight.vue';
import SlidingText from '@/components/common/SlidingText.vue';
import VirtualList from 'vue-virtual-draglist';
import LoadTrackListItem from './common/LoadTrackListItem.vue';

const panelVisible = computed(() => tracksVModel.value.length);
const currentIndex = ref(null);
const playingContext = ref({
  text: 'Currently Playing'
});

const player = usePlayerStore();

const tracksVModel = ref([]);

const handleClick = (index) => {
  player.loadTrackIndex(index);
}

const trackContextMenuRef = ref(null);

const openContextMenu = (event, track, trackIndex) => {
  console.log('Open track context menu:', track, trackIndex);
  trackContextMenuRef.value.openMenu(event, track, trackIndex);
}

const seekPlaybackHistory = (direction) => {
  if (direction == 1) {
    player.goToNextPlaylist();
  } else {
    player.goToPreviousPlaylist();
  }
}

const handleDrop = (event) => {
  const { newIndex, oldIndex } = event;
  console.log(event);
  player.moveTrack(oldIndex, newIndex);
}

const computeNextPlaylistButtonClasses = computed(() => {
  return {
    playbackHistoryIcon: true,
    scaleClickFeedback: player.canGoToNextPlaylist,
    disabledPlaybackHistoryButton: !player.canGoToNextPlaylist,
  }
});

const computePreviousPlaylistButtonClasses = computed(() => {
  return {
    playbackHistoryIcon: true,
    scaleClickFeedback: player.canGoToPreviousPlaylist,
    disabledPlaybackHistoryButton: !player.canGoToPreviousPlaylist,
  }
});

watch(
  () => player.currentTrackIndex,
  (index) => {
    currentIndex.value = index;
  },
  { immediate: true }
)

watch(
  () => player.currentPlaylist,
  (playlist) => {
    if (playlist && playlist.tracksIds) {

      let playingContextText = playlist.type;
      if (playlist.type == player.PLAYBACK_CONTEXTS.album) {
        playingContextText = 'Album: ' + playlist.context.name;
      } else if (playlist.type == player.PLAYBACK_CONTEXTS.userPlaylist) {
        playingContextText = 'Playlist: ' + playlist.context.name;
      } else if (playlist.type == player.PLAYBACK_CONTEXTS.userMix) {
        playingContextText = "Your mix";
      }
      playingContext.value.text = playingContextText;

      const seenTrackCounter = {};
      console.log("CurrentlyPlayingSidebar new playlist with .tracksIds", playlist.tracksIds);
      tracksVModel.value = playlist.tracksIds.map((trackId) => {
        const seenCount = seenTrackCounter[trackId] || 0;
        seenTrackCounter[trackId] = seenCount + 1;
        return {
          id: trackId,
          listItemId: trackId + seenCount,
        };
      });
    } else {
      tracksVModel.value = [];
    }
  },
  { immediate: true }
);

</script>

<style scoped>
.containero {
  display: flex;
  flex-direction: column;
}

.header {
  padding: 8px 16px;
  display: flex;
  flex-direction: row;
}

.currentlyPlaylistHeader {
  width: 0;
  flex: 1;
  overflow: hidden;
  text-align: center;
}

.currentlyPlayingText {
  font-size: 24px;
  white-space: nowrap;
  justify-content: space-around;
}

.trackRowsContainer {
  display: flex;
  width: 100%;
  flex-direction: column;
  overflow-y: auto;
  flex: 1;
}

.playbackHistoryIcon {
  fill: white;
  height: 32px;
  width: 32px;
}

.disabledPlaybackHistoryButton {
  opacity: 0.5;
}

.trackRow {
  display: flex;
  width: 100%;
  flex-direction: row;
  cursor: pointer;
  padding: 8px 16px;
  align-items: center;
}

.trackRow:hover {
  background-color: var(--highlighted-panel-color);
}

.trackImage {
  width: 40px;
  height: 40px;
}

.namesColumn {
  flex: 1;
  width: 0;
  display: flex;
  flex-direction: column;
  padding: 0 8px;
}

.currentlyPlayingRow {
  color: var(--accent-color);
  text-decoration: bold;
}
</style>
