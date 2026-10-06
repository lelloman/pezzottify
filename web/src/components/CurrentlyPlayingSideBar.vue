<template>
  <aside class="sidebarContainer" aria-label="Playback queue">
    <header class="header">
      <h2>Queue</h2>
      <div class="historyActions" aria-label="Playback history">
        <button
          class="navButton"
          @click="seekPlaybackHistory(-1)"
          :disabled="!playback.canGoToPreviousPlaylist"
          aria-label="Previous playlist"
          title="Previous playlist"
        >
          <ChevronLeft class="navIcon" />
        </button>
        <button
          class="navButton"
          @click="seekPlaybackHistory(1)"
          :disabled="!playback.canGoToNextPlaylist"
          aria-label="Next playlist"
          title="Next playlist"
        >
          <ChevronRight class="navIcon" />
        </button>
      </div>
    </header>
    <section
      v-if="playingContext"
      class="queueContext"
      aria-label="Playback context"
    >
      <div class="contextType">
        <span>{{ playingContext.kind }}</span>
        <span
          v-if="playback.currentPlaylist?.context?.edited"
          class="editedBadge"
          >Edited</span
        >
      </div>
      <RouterLink
        v-if="playingContext.to"
        :to="playingContext.to"
        class="contextName"
        :title="playingContext.name"
        >{{ playingContext.name }}</RouterLink
      >
      <p v-else class="contextName" :title="playingContext.name">
        {{ playingContext.name }}
      </p>
      <RouterLink
        v-if="destination.length"
        to="/steering"
        class="destinationLink"
        :title="
          destination.map((item) => item.label || item.entity_id).join(', ')
        "
      >
        <SteeringWheelIcon />
        <span class="destinationText"
          ><span class="destinationLabel">Destination</span
          ><span class="destinationName">{{ destinationTitle }}</span></span
        >
        <ChevronRight class="destinationArrow" />
      </RouterLink>
      <div v-if="destination.length" class="destinationProgress">
        <div
          role="progressbar"
          aria-label="Steering progress"
          :aria-valuenow="destinationProgress"
          aria-valuemin="0"
          aria-valuemax="100"
          class="progressTrack"
        >
          <span :style="{ width: destinationProgress + '%' }" />
        </div>
        <span>{{
          user.isSmartContinuationEnabled
            ? `${gravity.steps_done || 0} of ${gravity.steps_total} tracks`
            : "Smart continuation is off"
        }}</span>
      </div>
    </section>
    <details v-if="previousTracks.length" class="previousSection">
      <summary>
        <ChevronRight class="historyChevron" />
        <span>Previously played</span>
        <span class="historyCount">{{ previousTracks.length }}</span>
      </summary>
      <VirtualList
        v-model="previousTracks"
        class="previousTrackList"
        :style="{
          height: `min(${Math.min(previousTracks.length * 64, 192)}px, 24vh)`,
        }"
        data-key="listItemId"
        :keeps="10"
        :size="64"
        :disabled="true"
        aria-label="Earlier tracks in this queue"
      >
        <template v-slot:item="{ record, index }">
          <QueueTrackRow
            :trackId="record.id"
            :isAuto="record.isAuto"
            @play="handleClick(index)"
            @menu="openContextMenu($event, record.id, index)"
          />
        </template>
      </VirtualList>
    </details>
    <section v-if="currentTrackId" class="nowPlaying">
      <h3>Now playing</h3>
      <QueueTrackRow
        :trackId="currentTrackId"
        current
        @play="handleClick(currentIndex)"
        @menu="openContextMenu($event, currentTrackId, currentIndex)"
      />
    </section>
    <div class="nextHeading">
      <h3>Next up</h3>
    </div>
    <div v-if="tracksVModel.length" class="trackList">
      <VirtualList
        v-model="tracksVModel"
        class="queueVirtualList"
        data-key="listItemId"
        :keeps="30"
        :size="64"
        @drop="handleDrop"
      >
        <template v-slot:item="{ record, index }">
          <QueueTrackRow
            :trackId="record.id"
            :isAuto="record.isAuto"
            @play="handleClick(index + upcomingOffset)"
            @menu="openContextMenu($event, record.id, index + upcomingOffset)"
          />
        </template>
      </VirtualList>
    </div>
    <p v-else class="emptyQueue">
      {{
        currentTrackId
          ? "You’ve reached the end of the queue."
          : "Play an album, playlist, or track to start your queue."
      }}
    </p>
    <Teleport to="body"
      ><TrackContextMenu ref="trackContextMenuRef" :canRemoveFromQueue="true"
    /></Teleport>
  </aside>
</template>
<script setup>
import "@/assets/main.css";
import { watch, ref, computed } from "vue";
import { usePlaybackStore } from "@/store/playback";
import { useUserStore } from "@/store/user";
import { mixTitle } from "@/utils/steeringArt";
import { progress } from "@/utils/gravity";
import SteeringWheelIcon from "./icons/SteeringWheelIcon.vue";
import TrackContextMenu from "@/components/common/contextmenu/TrackContextMenu.vue";
import ChevronLeft from "@/components/icons/ChevronLeft.vue";
import ChevronRight from "@/components/icons/ChevronRight.vue";

import VirtualList from "vue-virtual-draglist";
import QueueTrackRow from "./common/QueueTrackRow.vue";

const currentIndex = ref(null);

const playback = usePlaybackStore();
const user = useUserStore();
const gravity = computed(() => playback.currentGravity);
const destination = computed(() => gravity.value?.destination || []);
const destinationTitle = computed(() => mixTitle(destination.value));
const destinationProgress = computed(() =>
  Math.round(progress(gravity.value) * 100),
);
const entityRoute = (type, id) =>
  id && ["album", "artist", "track", "playlist", "work", "genre"].includes(type)
    ? `/${type}/${encodeURIComponent(id)}`
    : null;
const playingContext = computed(() => {
  const playlist = playback.currentPlaylist;
  if (!playlist) return null;
  const context = playlist.context || {};
  const types = playback.PLAYBACK_CONTEXTS;
  if (playlist.type === types.album)
    return {
      kind: "Album",
      name: context.name || "Album",
      to: entityRoute("album", context.id),
    };
  if (playlist.type === types.userPlaylist)
    return {
      kind: "Playlist",
      name: context.name || "Playlist",
      to: entityRoute("playlist", context.id),
    };
  if (playlist.type === types.radio) {
    const kinds = {
      greatest_hits: "Greatest hits",
      work_versions: "All versions",
      custom: "Custom radio",
      genre: "Genre radio",
    };
    return {
      kind: kinds[context.source] || "Radio",
      name: context.seed?.label || context.name || "Radio",
      to: entityRoute(context.seed?.entity_type, context.seed?.entity_id),
    };
  }
  return { kind: "Mix", name: context.name || "Your mix", to: null };
});

const tracksVModel = ref([]);
const previousTracks = ref([]);
const currentTrackId = computed(() => playback.currentTrackId);
const upcomingOffset = computed(() =>
  Number.isInteger(playback.currentTrackIndex)
    ? playback.currentTrackIndex + 1
    : 0,
);

const handleClick = (index) => {
  playback.loadTrackIndex(index);
  playback.play();
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
  playback.moveTrack(
    oldIndex + upcomingOffset.value,
    newIndex + upcomingOffset.value,
  );
};

watch(
  () => playback.currentTrackIndex,
  (index) => {
    currentIndex.value = index;
  },
  { immediate: true },
);

const buildTrackRows = (trackIds, autoTrackIds = []) => {
  const seenTrackCounter = {};
  const autoSet = new Set(autoTrackIds);
  return trackIds.map((trackId) => {
    const seenCount = seenTrackCounter[trackId] || 0;
    seenTrackCounter[trackId] = seenCount + 1;
    return {
      id: trackId,
      listItemId: `${trackId}:${seenCount}`,
      isAuto: autoSet.has(trackId),
    };
  });
};

watch(
  () => [
    playback.currentPlaylist?.tracksIds || [],
    playback.currentPlaylist?.gravity?.auto_track_ids || [],
    upcomingOffset.value,
  ],
  ([trackIds, autoTrackIds, offset]) => {
    const rows = buildTrackRows(trackIds, autoTrackIds);
    previousTracks.value = rows.slice(0, Math.max(0, offset - 1));
    tracksVModel.value = rows.slice(offset);
  },
  { immediate: true, deep: true },
);
</script>

<style scoped>
.sidebarContainer {
  display: flex;
  flex-direction: column;
  min-height: 0;
  height: 100%;
  overflow: hidden;
  background: var(--surface-panel);
  border-radius: 8px;
}
.header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px;
  flex-shrink: 0;
}
.header h2,
h3 {
  font-size: 16px;
  font-weight: 700;
  margin: 0;
}
.historyActions {
  display: flex;
  gap: 4px;
}
.navButton {
  width: 32px;
  height: 32px;
  display: grid;
  place-items: center;
  border-radius: 50%;
  color: var(--text-subdued);
}
.navButton:hover:not(:disabled) {
  color: white;
  transform: scale(1.04);
}
.navButton:disabled {
  opacity: 0.3;
  cursor: default;
}
.navButton:focus-visible {
  outline: 2px solid white;
  outline-offset: 2px;
}
.navIcon {
  width: 16px;
  height: 16px;
  fill: currentColor;
}
.queueContext {
  margin: 0 16px 8px;
  padding: 0 0 16px;
  border-bottom: 1px solid var(--surface-border);
  flex-shrink: 0;
  max-height: 25vh;
  overflow: auto;
}
.contextType {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--text-subdued);
  font-size: 12px;
  margin-bottom: 4px;
}
.editedBadge {
  padding: 1px 6px;
  border-radius: 4px;
  background: var(--surface-hover);
}
.contextName {
  display: block;
  margin: 0;
  font-size: 16px;
  line-height: 22px;
  font-weight: 700;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  color: var(--text-base);
  text-decoration: none;
}
a.contextName:hover {
  text-decoration: underline;
}
.destinationLink {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-top: 12px;
  color: var(--text-base);
  text-decoration: none;
  border-radius: 4px;
}
.destinationLink > svg {
  width: 20px;
  height: 20px;
  fill: currentColor;
  flex-shrink: 0;
  color: var(--text-subdued);
}
.destinationText {
  min-width: 0;
  flex: 1;
  display: flex;
  flex-direction: column;
}
.destinationLabel {
  font-size: 12px;
  color: var(--text-subdued);
}
.destinationName {
  font-size: 14px;
  line-height: 20px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.destinationLink:hover .destinationName {
  text-decoration: underline;
}
.destinationLink > .destinationArrow {
  width: 16px;
  height: 16px;
}
.destinationProgress {
  font-size: 12px;
  color: var(--text-subdued);
  margin: 8px 0 0 30px;
}
.progressTrack {
  height: 3px;
  border-radius: 3px;
  overflow: hidden;
  background: var(--surface-hover);
  margin-bottom: 4px;
}
.progressTrack span {
  display: block;
  height: 100%;
  background: var(--spotify-green);
}
.queueContext a:focus-visible {
  outline: 2px solid white;
  outline-offset: 2px;
}
.previousSection {
  flex-shrink: 0;
  min-width: 0;
}
.previousSection summary {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 16px;
  cursor: pointer;
  list-style: none;
  color: var(--text-subdued);
  font-size: 14px;
  font-weight: 700;
}
.previousSection summary::-webkit-details-marker {
  display: none;
}
.previousSection summary:hover {
  color: var(--text-base);
}
.previousSection summary:focus-visible {
  outline: 2px solid white;
  outline-offset: -4px;
  border-radius: 4px;
}
.historyChevron {
  width: 16px;
  height: 16px;
  fill: currentColor;
  flex-shrink: 0;
}
.previousSection[open] .historyChevron {
  transform: rotate(90deg);
}
.historyCount {
  margin-left: auto;
  font-size: 12px;
  font-weight: 400;
}
.previousTrackList {
  height: min(192px, 24vh);
  padding: 0 8px;
}
.nowPlaying {
  padding: 12px 8px 16px;
  flex-shrink: 0;
}
.nowPlaying h3 {
  margin: 0 8px 8px;
}
.nextHeading {
  padding: 12px 16px 8px;
  flex-shrink: 0;
  min-width: 0;
}
.nextHeading p {
  font-size: 14px;
  line-height: 20px;
  color: var(--text-subdued);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  margin: 4px 0 0;
}
.trackList {
  flex: 1;
  min-height: 0;
  overflow: hidden;
}
.queueVirtualList {
  height: 100%;
  padding: 0 8px 8px;
}
.emptyQueue {
  padding: 8px 16px 24px;
  font-size: 14px;
  color: var(--text-subdued);
  line-height: 1.5;
}
</style>
