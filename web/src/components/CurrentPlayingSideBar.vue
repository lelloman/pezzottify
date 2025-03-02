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
      <div class="trackRow" v-for="(track, index) in tracks" :class="{ currentlyPlayingRow: index == currentIndex }"
        :key="index" @click.stop="handleClick(index)" @contextmenu.prevent="openContextMenu($event, track)">
        <MultiSourceImage class="trackImage scaleClickFeedback" :urls="track ? track.imageUrls : []"
          @click.stop="handleClickOnTrackImage(track)" />
        <div class="namesColumn">
          <TrackName v-if="track" :track="track" :hoverAnimation="true" />
          <ClickableArtistsNames :artistsIdsNames="track ? track.artists : []" />
        </div>
        <p>{{ track ? formatDuration(track.duration) : '' }} </p>
      </div>
    </div>

    <TrackContextMenu ref="trackContextMenuRef" />
  </div>
</template>
<script setup>
import '@/assets/main.css';
import { watch, ref, computed } from 'vue';
import { usePlayerStore } from '@/store/player';
import { formatDuration } from '@/utils';
import { useRouter } from 'vue-router';
import MultiSourceImage from '@/components/common/MultiSourceImage.vue';
import ClickableArtistsNames from '@/components/common/ClickableArtistsNames.vue';
import TrackName from '@/components/common/TrackName.vue';
import TrackContextMenu from '@/components/common/contextmenu/TrackContextMenu.vue';
import ChevronLeft from '@/components/icons/ChevronLeft.vue';
import ChevronRight from '@/components/icons/ChevronRight.vue';
import SlidingText from '@/components/common/SlidingText.vue';

const panelVisible = computed(() => tracks.value.length);
const tracks = ref([]);
const currentIndex = ref(null);
const playingContext = ref({
  text: 'Currently Playing'
});

const router = useRouter();
const player = usePlayerStore();

const handleClick = (index) => {
  player.loadTrackIndex(index);
}

const handleClickOnTrackImage = (track) => {
  router.push("/album/" + track.albumId);
}

const trackContextMenuRef = ref(null);

const openContextMenu = (event, track) => {
  trackContextMenuRef.value.openMenu(event, track);
}

const seekPlaybackHistory = (direction) => {
  if (direction == 1) {
    player.goToNextPlaylist();
  } else {
    player.goToPreviousPlaylist();
  }
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
    if (playlist && playlist.tracks) {

      let playingContextText = playlist.type;
      if (playlist.type == player.PLAYBACK_CONTEXTS.album) {
        playingContextText = 'Album: ' + playlist.context.name;
      } else if (playlist.type == player.PLAYBACK_CONTEXTS.userPlaylist) {
        playingContextText = 'Playlist: ' + playlist.context.name;
      } else if (playlist.type == player.PLAYBACK_CONTEXTS.userMix) {
        playingContextText = "Your mix";
      }
      playingContext.value.text = playingContextText;

      tracks.value = playlist.tracks.map((track) => {
        return track;
      });
    } else {
      tracks.value = [];
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
  align-content: center;
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
