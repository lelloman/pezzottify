<template>
  <div v-if="panelVisible" class="sidebarContainer">
    <div class="header">
      <button
        :class="computePreviousPlaylistButtonClasses"
        @click.stop="seekPlaybackHistory(-1)"
        :disabled="!playback.canGoToPreviousPlaylist"
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
        :disabled="!playback.canGoToNextPlaylist"
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
              @track-clicked="handleClick(index)"
              :isCurrentlyPlaying="index == currentIndex"
              :minimal="true"
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
import { usePlaybackStore } from "@/store/playback";
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

const playback = usePlaybackStore();

const tracksVModel = ref([]);

const handleClick = (index) => {
  playback.loadTrackIndex(index);
};

const trackContextMenuRef = ref(null);

const openContextMenu = (event, track, trackIndex) => {
  console.log("Open track context menu:", track, trackIndex);
  trackContextMenuRef.value.openMenu(event, track, trackIndex);
};

const seekPlaybackHistory = (direction) => {
  if (direction == 1) {
    playback.goToNextPlaylist();
  } else {
    playback.goToPreviousPlaylist();
  }
};

const handleDrop = (event) => {
  const { newIndex, oldIndex } = event;
  console.log(event);
  playback.moveTrack(oldIndex, newIndex);
};

const computeNextPlaylistButtonClasses = computed(() => {
  return {
    navButton: true,
    navButtonEnabled: playback.canGoToNextPlaylist,
    navButtonDisabled: !playback.canGoToNextPlaylist,
  };
});

const computePreviousPlaylistButtonClasses = computed(() => {
  return {
    navButton: true,
    navButtonEnabled: playback.canGoToPreviousPlaylist,
    navButtonDisabled: !playback.canGoToPreviousPlaylist,
  };
});

watch(
  () => playback.currentTrackIndex,
  (index) => {
    currentIndex.value = index;
  },
  { immediate: true },
);

watch(
  () => playback.currentPlaylist,
  (playlist) => {
    if (playlist && playlist.tracksIds) {
      let playingContextText = playlist.type;
      if (playlist.type == playback.PLAYBACK_CONTEXTS.album) {
        playingContextText = "Album: " + playlist.context.name;
      } else if (playlist.type == playback.PLAYBACK_CONTEXTS.userPlaylist) {
        playingContextText = "Playlist: " + playlist.context.name;
      } else if (playlist.type == playback.PLAYBACK_CONTEXTS.userMix) {
        playingContextText = "Your mix";
      } else if (playlist.type == playback.PLAYBACK_CONTEXTS.radio) {
        const seedLabel = playlist.context?.seed?.label || "Radio";
        if (playlist.context?.source === "custom") {
          playingContextText = "Custom radio: " + seedLabel;
        } else if (playlist.context?.source === "genre") {
          playingContextText = "Genre radio: " + seedLabel;
        } else {
          playingContextText = "Radio: " + seedLabel;
        }
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
  background: var(--surface-panel);
  border: 1px solid var(--surface-border);
  border-radius: 8px;
  box-shadow: var(--shadow-sm);
}

/* Header */
.header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px;
  border-bottom: 1px solid var(--surface-border);
}

.headerTitle {
  flex: 1;
  min-width: 0;
  text-align: center;
}

.playlistName {
  font-size: 0.88rem;
  font-weight: 850;
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
  border-radius: 7px;
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
  background: var(--surface-hover);
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
  padding: 8px 0;
}

.trackItem {
  display: flex;
  align-items: center;
  padding: 4px 8px;
  cursor: pointer;
  border-radius: 7px;
  margin: 0 8px;
  transition: background-color var(--transition-fast);
}

.trackItem:hover {
  background-color: var(--surface-hover);
}

.trackItem.isPlaying {
  background-color: var(--surface-active);
}

.trackItem.isPlaying:hover {
  background-color: var(--surface-hover);
}
</style>
