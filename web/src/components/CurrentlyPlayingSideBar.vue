<template>
  <div v-if="panelVisible" class="sidebarContainer">
    <div class="header">
      <button
        :class="computePreviousPlaylistButtonClasses"
        @click.stop="seekPlaybackHistory(-1)"
        :disabled="!player.canGoToPreviousPlaylist"
        aria-label="Previous playlist"
      >
        <ChevronLeft class="navIcon" />
      </button>
      <div class="headerTitle">
        <SlidingText :hoverAnimation="true">
          <span class="playlistName">{{ playingContext.text }}</span>
        </SlidingText>
      </div>
      <button
        :class="computeNextPlaylistButtonClasses"
        @click.stop="seekPlaybackHistory(1)"
        :disabled="!player.canGoToNextPlaylist"
        aria-label="Next playlist"
      >
        <ChevronRight class="navIcon" />
      </button>
    </div>
    <div class="trackList">
      <VirtualList
        v-model="tracksVModel"
        data-key="listItemId"
        @drop="handleDrop"
      >
        <template v-slot:item="{ record, index }">
          <div
            :class="['trackItem', { isPlaying: index == currentIndex }]"
            @contextmenu.prevent="openContextMenu($event, record.id, index)"
          >
            <LoadTrackListItem
              :trackId="record.id"
              :trackNumber="index + 1"
              @track-clicked="handleClick(index)"
              :isCurrentlyPlaying="index == currentIndex"
            />
          </div>
        </template>
      </VirtualList>
    </div>

    <TrackContextMenu ref="trackContextMenuRef" :canRemoveFromQueue="true" />
  </div>
</template>
<script setup>
import "@/assets/main.css";
import { watch, ref, computed } from "vue";
import { usePlayerStore } from "@/store/player";
import TrackContextMenu from "@/components/common/contextmenu/TrackContextMenu.vue";
import ChevronLeft from "@/components/icons/ChevronLeft.vue";
import ChevronRight from "@/components/icons/ChevronRight.vue";
import SlidingText from "@/components/common/SlidingText.vue";
import VirtualList from "vue-virtual-draglist";
import LoadTrackListItem from "./common/LoadTrackListItem.vue";

const panelVisible = computed(() => tracksVModel.value.length);
const currentIndex = ref(null);
const playingContext = ref({
  text: "Currently Playing",
});

const player = usePlayerStore();

const tracksVModel = ref([]);

const handleClick = (index) => {
  player.loadTrackIndex(index);
};

const trackContextMenuRef = ref(null);

const openContextMenu = (event, track, trackIndex) => {
  console.log("Open track context menu:", track, trackIndex);
  trackContextMenuRef.value.openMenu(event, track, trackIndex);
};

const seekPlaybackHistory = (direction) => {
  if (direction == 1) {
    player.goToNextPlaylist();
  } else {
    player.goToPreviousPlaylist();
  }
};

const handleDrop = (event) => {
  const { newIndex, oldIndex } = event;
  console.log(event);
  player.moveTrack(oldIndex, newIndex);
};

const computeNextPlaylistButtonClasses = computed(() => {
  return {
    navButton: true,
    navButtonEnabled: player.canGoToNextPlaylist,
    navButtonDisabled: !player.canGoToNextPlaylist,
  };
});

const computePreviousPlaylistButtonClasses = computed(() => {
  return {
    navButton: true,
    navButtonEnabled: player.canGoToPreviousPlaylist,
    navButtonDisabled: !player.canGoToPreviousPlaylist,
  };
});

watch(
  () => player.currentTrackIndex,
  (index) => {
    currentIndex.value = index;
  },
  { immediate: true },
);

watch(
  () => player.currentPlaylist,
  (playlist) => {
    if (playlist && playlist.tracksIds) {
      let playingContextText = playlist.type;
      if (playlist.type == player.PLAYBACK_CONTEXTS.album) {
        playingContextText = "Album: " + playlist.context.name;
      } else if (playlist.type == player.PLAYBACK_CONTEXTS.userPlaylist) {
        playingContextText = "Playlist: " + playlist.context.name;
      } else if (playlist.type == player.PLAYBACK_CONTEXTS.userMix) {
        playingContextText = "Your mix";
      }
      playingContext.value.text = playingContextText;

      const seenTrackCounter = {};
      console.log(
        "CurrentlyPlayingSidebar new playlist with .tracksIds",
        playlist.tracksIds,
      );
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
  { immediate: true },
);
</script>

<style scoped>
.sidebarContainer {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: linear-gradient(180deg, var(--bg-highlight) 0%, var(--bg-elevated) 100px);
  border-radius: var(--radius-lg);
}

/* Header */
.header {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
  padding: var(--spacing-4);
  border-bottom: 1px solid var(--border-subtle);
}

.headerTitle {
  flex: 1;
  min-width: 0;
  text-align: center;
}

.playlistName {
  font-size: var(--text-lg);
  font-weight: var(--font-semibold);
  color: var(--text-base);
  white-space: nowrap;
}

/* Navigation Buttons */
.navButton {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  padding: 0;
  border: none;
  border-radius: var(--radius-full);
  background: transparent;
  cursor: pointer;
  transition: all var(--transition-fast);
}

.navButton:focus-visible {
  outline: 2px solid var(--spotify-green);
  outline-offset: 2px;
}

.navButtonEnabled {
  color: var(--text-base);
}

.navButtonEnabled:hover {
  background: var(--bg-press);
  transform: scale(1.1);
}

.navButtonEnabled:active {
  transform: scale(0.95);
}

.navButtonDisabled {
  color: var(--text-subtle);
  cursor: not-allowed;
}

.navIcon {
  width: 20px;
  height: 20px;
  fill: currentColor;
}

/* Track List */
.trackList {
  flex: 1;
  overflow-y: auto;
  padding: var(--spacing-2) 0;
}

.trackItem {
  display: flex;
  align-items: center;
  padding: var(--spacing-1) var(--spacing-3);
  cursor: pointer;
  border-radius: var(--radius-md);
  margin: 0 var(--spacing-2);
  width: calc(100% - var(--spacing-4));
  transition: background-color var(--transition-fast);
}

.trackItem:hover {
  background-color: var(--bg-highlight);
}

.trackItem.isPlaying {
  background: linear-gradient(90deg, rgba(29, 185, 84, 0.15) 0%, transparent 100%);
  border-left: 3px solid var(--spotify-green);
  margin-left: calc(var(--spacing-2) - 3px);
}
</style>
