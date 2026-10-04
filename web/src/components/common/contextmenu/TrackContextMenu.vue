<template>
  <LyricsDownloadNotice ref="lyricsNotice" />
  <ContextMenu ref="contextMenu" :items="visibleMenuItems" />
  <RadioBuilderModal
    :isOpen="showRadioBuilder"
    seedEntityType="track"
    :seedEntityId="trackId || ''"
    @close="showRadioBuilder = false"
  />
  <DestinationStepsPrompt
    :isOpen="destinationReference !== null"
    :reference="destinationReference"
    @close="destinationReference = null"
  />
</template>

<script setup>
import LyricsDownloadNotice from "@/components/common/LyricsDownloadNotice.vue";
import DownloadIcon from "@/components/icons/DownloadIcon.vue";
import PlusIcon from "@/components/icons/PlusIcon.vue";
import ContextMenu from "@/components/common/contextmenu/ContextMenu.vue";
import { computed, ref, markRaw } from "vue";
import { MAX_COMPONENTS } from "@/utils/gravity";
import PlaylistPlusIcon from "@/components/icons/PlaylistPlusIcon.vue";
import RadioIcon from "@/components/icons/RadioIcon.vue";
import { useUserStore } from "@/store/user";
import { usePlaybackStore } from "@/store/playback";
import PlaylistCancelIcon from "@/components/icons/PlaylistCancelIcon.vue";
import TrashOutlineIcon from "@/components/icons/TrashOutlineIcon.vue";
import RadioBuilderModal from "@/components/common/RadioBuilderModal.vue";
import DestinationStepsPrompt from "@/components/common/DestinationStepsPrompt.vue";
import SteeringWheelIcon from "@/components/icons/SteeringWheelIcon.vue";

const props = defineProps({
  canRemoveFromQueue: {
    type: Boolean,
    default: false,
  },
  canRemoveFromPlaylist: {
    type: Boolean,
    default: false,
  },
  contextId: {
    type: String,
    default: null,
  },
});

const contextMenu = ref(null);
const lyricsNotice = ref(null);
const userStore = useUserStore();
const playback = usePlaybackStore();

const trackId = ref(null);
const trackIndex = ref(null);
const showRadioBuilder = ref(false);
const destinationReference = ref(null);

const handleAddToQueueClick = () => {
  console.log("TrackContextMenu handleAddToQueueClick" + trackId.value);
  if (trackId.value) {
    playback.addTracksToPlaylist([trackId.value]);
  }
};

const makeAddToPlaylistSubMenu = () => {
  console.log(
    "Make add to playlist sub menu, userStore.playlistsData.list.length: ",
    userStore.playlistsData.list.length,
  );
  return userStore.playlistsData.list.map((playlist) => ({
    name: playlist.name,
    action: () =>
      userStore.addTracksToPlaylist(playlist.id, [trackId.value], () => {}),
  }));
};

const visibleMenuItems = computed(() =>
  menuItems.value.filter((item) => !item.visible || item.visible()),
);

const menuItems = ref([
  {
    icon: markRaw(DownloadIcon),
    name: "Download lyrics",
    action: () => lyricsNotice.value.download("track", trackId.value),
  },
  {
    icon: markRaw(PlusIcon),
    name: "Add to playlist",
    subMenu: makeAddToPlaylistSubMenu,
  },
]);

if (props.canRemoveFromPlaylist) {
  menuItems.value.push({
    icon: markRaw(TrashOutlineIcon),
    name: "Remove from this playlist",
    action: () => {
      console.log(
        "TrackContextMenu remove from playlist track index:" +
          trackIndex.value +
          " contextId:" +
          props.contextId,
      );
      if (Number.isInteger(trackIndex.value) && props.contextId) {
        userStore.removeTracksFromPlaylist(
          props.contextId,
          [trackIndex.value],
          () => {},
        );
      }
    },
  });
}

menuItems.value.push({
  icon: markRaw(RadioIcon),
  name: "Listen to radio",
  action: () => {
    if (trackId.value) {
      playback.setRadioFromItem("track", trackId.value);
    }
  },
});

menuItems.value.push({
  icon: markRaw(RadioIcon),
  name: "Customize radio",
  action: () => {
    if (trackId.value) {
      showRadioBuilder.value = true;
    }
  },
});

menuItems.value.push({
  icon: markRaw(SteeringWheelIcon),
  name: "Set as playback destination",
  action: () => {
    if (trackId.value) {
      destinationReference.value = {
        entity_type: "track",
        entity_id: trackId.value,
      };
    }
  },
});

menuItems.value.push({
  icon: markRaw(SteeringWheelIcon),
  name: "Add to playback destination",
  // Only offered when a destination mix exists and has room.
  visible: () => {
    const destination = playback.currentGravity?.destination;
    return Boolean(destination?.length && destination.length < MAX_COMPONENTS);
  },
  action: () => {
    if (trackId.value) {
      playback.addGravityDestinationComponent({
        entity_type: "track",
        entity_id: trackId.value,
      });
    }
  },
});

menuItems.value.push({
  icon: markRaw(PlaylistPlusIcon),
  name: "Add to queue",
  action: () => handleAddToQueueClick(),
});

if (props.canRemoveFromQueue) {
  menuItems.value.push({
    icon: markRaw(PlaylistCancelIcon),
    name: "Remove from queue",
    action: () => {
      if (Number.isInteger(trackIndex.value)) {
        playback.removeTrackFromPlaylist(trackIndex.value);
      }
    },
  });
}

const openMenu = (event, selectedTrackId, selectedTrackIndex) => {
  trackId.value = selectedTrackId;
  trackIndex.value = selectedTrackIndex;
  contextMenu.value.openMenu(event);
};

defineExpose({
  openMenu,
});
</script>

<style scoped>
@import "@/assets/icons.css";

.contextMenuItem {
  display: flex;
  flex-direction: row;
  padding: 8px;
  height: 50px;
  cursor: pointer;
  align-items: center;
  font-size: 14px;
  padding: 0 16px;
}

.contextMenuItem span {
  flex: 1;
}

.contextMenuItem:hover {
  background-color: #222;
}

.subMenu {
  z-index: 1001;
  position: fixed;
  width: 200px;
  border: 1px solid #ccc;
  background-color: #151515;
  z-index: 1001;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
</style>
