<template>
  <div class="detailPageHost">
    <div v-if="loading">Loading...</div>
    <DetailPage
      v-else-if="playlist"
      :title="playlist.name"
      kind="Playlist"
      :imageUrls="coverImages[0] || []"
    >
      <template v-if="coverImages.length" #artwork>
        <div
          class="playlistArtwork"
          :class="{ collage: coverImages.length > 1 }"
        >
          <MultiSourceImage
            v-for="(urls, index) in coverImages"
            :key="index"
            :urls="urls"
            :lazy="false"
            alt=""
          />
        </div>
      </template>
      <template #meta
        ><span
          >{{ playlist.tracks.length }} tracks<span v-if="playlistDuration">
            · {{ playlistDuration }} min</span
          ></span
        ></template
      >
      <template #actions
        ><DetailActions
          playLabel="Play playlist"
          :disabled="!playlist.tracks.length"
          @play="handleClickOnPlay"
          ><template #secondary>
            <button
              type="button"
              title="Rename playlist"
              @click="handleEditButtonClick"
            >
              <EditIcon /><span class="actionLabel">Rename playlist</span>
            </button>
            <button
              type="button"
              title="Delete playlist"
              @click="handleClickOnDelete"
            >
              <TrashIcon class="deleteIcon" /><span class="actionLabel"
                >Delete playlist</span
              >
            </button>
          </template></DetailActions
        ></template
      >
      <div v-if="playlist.tracks.length" class="tracksSection">
        <div class="detailTrackHeading">
          <span>#</span><span>Title</span><span>Duration</span>
        </div>
        <div
          v-for="(trackId, trackIndex) in playlist.tracks"
          :key="trackIndex + trackId"
          class="track"
          @contextmenu.prevent="
            openTrackContextMenu($event, trackId, trackIndex)
          "
        >
          <LoadTrackListItem
            albumLayout
            :contextId="playlistId"
            :trackId="trackId"
            :trackNumber="trackIndex + 1"
            @track-clicked="handleTrackSelection(trackIndex)"
            :isCurrentlyPlaying="trackIndex == currentTrackIndex"
          />
        </div>
      </div>
      <p v-else class="emptyPlaylist">
        This playlist is empty. Add tracks from their context menu to get
        started.
      </p>
    </DetailPage>
    <div v-else-if="error">Error. {{ error }}</div>
  </div>

  <Transition>
    <ConfirmationDialog
      v-if="deleteConfirmationDialogOpen"
      :isOpen="deleteConfirmationDialogOpen"
      :closeCallback="() => (deleteConfirmationDialogOpen = false)"
      :title="'Delete playlist'"
      :positiveButtonCallback="handleDeletePlaylistConfirmation"
    >
      <template #message>
        Are you sure you want to delete playlist
        <span style="font-weight: bold">{{ playlist?.name }}</span
        >?
      </template>
    </ConfirmationDialog>
  </Transition>

  <Transition>
    <ConfirmationDialog
      v-if="isEditMode"
      :isOpen="isEditMode"
      :closeCallback="closeEditMode"
      :title="'Edit playlist name'"
      :negativeButtonText="'Cancel'"
      :positiveButtonText="'Save'"
      :positiveButtonCallback="handleChangeNameButtonClicked"
    >
      <template #message>
        <input id="editPlaylistNameInput" aria-label="Playlist name" />
      </template>
    </ConfirmationDialog>
  </Transition>

  <TrackContextMenu
    :contextId="playlistId"
    :canRemoveFromPlaylist="true"
    ref="trackContextMenuRef"
  />
</template>

<script setup>
import { watch, ref, computed, onBeforeUnmount } from "vue";
import MultiSourceImage from "@/components/common/MultiSourceImage.vue";
import { useStaticsStore } from "@/store/statics";
import { chooseAlbumCoverImageUrl } from "@/utils";
import DetailPage from "@/components/common/DetailPage.vue";
import DetailActions from "@/components/common/DetailActions.vue";

import TrashIcon from "../icons/TrashIcon.vue";
import ConfirmationDialog from "@/components/common/ConfirmationDialog.vue";
import { useRoute, useRouter } from "vue-router";
import { useUserStore } from "@/store/user";
import EditIcon from "@/components/icons/EditIcon.vue";
import LoadTrackListItem from "@/components/common/LoadTrackListItem.vue";
import { usePlaybackStore } from "@/store/playback";
import TrackContextMenu from "@/components/common/contextmenu/TrackContextMenu.vue";

// Define playlistId prop
const props = defineProps({
  playlistId: {
    type: String,
    required: true,
  },
});

const router = useRouter();
const route = useRoute();
const userStore = useUserStore();
const playback = usePlaybackStore();
const statics = useStaticsStore();
const trackRefs = new Map();
const trackData = (id) => {
  if (!trackRefs.has(id)) trackRefs.set(id, statics.getTrack(id));
  return trackRefs.get(id).item;
};
const coverImages = computed(() => {
  const tracks = playlist.value?.tracks || [];
  const count = Math.min(4, tracks.length);
  const albums = [
    ...new Set(
      Array.from(
        { length: count },
        (_, i) =>
          trackData(tracks[Math.floor((i * tracks.length) / count)])?.album_id,
      ).filter(Boolean),
    ),
  ];
  const images = albums.map((id) => chooseAlbumCoverImageUrl({ id }));
  return images.length > 1
    ? Array.from({ length: 4 }, (_, i) => images[i % images.length])
    : images;
});
const playlistDuration = computed(() => {
  const durations = (playlist.value?.tracks || []).map(
    (id) => trackData(id)?.duration,
  );
  return durations.length && durations.every(Number.isFinite)
    ? Math.round(durations.reduce((sum, duration) => sum + duration, 0) / 60000)
    : null;
});

const loading = ref(true);
const error = ref(null);
const playlistRef = ref(null);
const trackContextMenuRef = ref(null);
const currentTrackIndex = ref(null);

const deleteConfirmationDialogOpen = ref(false);
const isEditMode = ref(false);

const handleEditButtonClick = () => {
  router.push({ query: { edit: !isEditMode.value } });
};

const playlist = computed(() => {
  return playlistRef.value?.value;
});

const openTrackContextMenu = (event, trackId, trackIndex) => {
  console.log("Open track context menu:", trackId, trackIndex);
  trackContextMenuRef.value.openMenu(event, trackId, trackIndex);
};

const handleChangeNameButtonClicked = async () => {
  const newName = document.getElementById("editPlaylistNameInput").value;
  closeEditMode();
  userStore.updatePlaylistName(props.playlistId, newName, () => {});
};

const closeEditMode = () => {
  router.push({});
};

const handleDeletePlaylistConfirmation = async () => {
  deleteConfirmationDialogOpen.value = false;
  userStore.deletePlaylist(props.playlistId, () => router.push("/"));
};

const handleClickOnPlay = () => {
  console.log("Play playlist:", props.playlistId);
  playback.setUserPlaylist(playlist.value);
};

const handleClickOnDelete = () => {
  console.log("Delete playlist:", props.playlistId);
  deleteConfirmationDialogOpen.value = true;
};

const handleTrackSelection = (index) =>
  playback.setUserPlaylist(playlist.value, index);

watch(
  [() => playback.currentTrackIndex, () => playback.currentPlaylist],
  ([newTrackIndex, newPlaylist]) => {
    console.log(
      "UserPlaylist.vue watcher - TrackIndex:",
      newTrackIndex,
      "Playlist:",
      newPlaylist,
      "PlaylistId:",
      props.playlistId,
    );
    if (
      newPlaylist &&
      newPlaylist.context &&
      newPlaylist.context.id === props.playlistId &&
      newPlaylist.context.edited === false &&
      Number.isInteger(newTrackIndex)
    ) {
      console.log(
        "UserPlaylist.vue - Setting currentTrackIndex to:",
        newTrackIndex,
      );
      currentTrackIndex.value = newTrackIndex;
    } else {
      currentTrackIndex.value = null;
    }
  },
  { immediate: true },
);

watch(
  route,
  (newRoute) => {
    isEditMode.value = newRoute.query.edit ? true : false;
    if (isEditMode.value && playlist.value) {
      setTimeout(() => {
        document.getElementById("editPlaylistNameInput").value =
          playlist.value.name;
        document.getElementById("editPlaylistNameInput").focus();
      }, 100);
    }
  },
  { immediate: true },
);

// Watch for playlist ID changes to load data
watch(
  () => props.playlistId,
  (newId) => {
    console.log("UserPlaylist Load playlist:", newId);
    if (newId) {
      if (playlistRef.value) {
        userStore.putPlaylistRef(playlistRef.value.id);
      }
      playlistRef.value = userStore.getPlaylistRef(newId);
      console.log("UserPlaylist got playlist ref");
      console.log(playlistRef.value);
      userStore.loadPlaylistData(newId).finally(() => (loading.value = false));
    }
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  // Release the reference when component is unmounted
  if (playlistRef.value) {
    userStore.putPlaylistRef(props.playlistId);
  }
});
</script>

<style scoped>
.deleteIcon :deep(path) {
  fill: currentColor;
}
.tracksSection {
  display: flex;
  flex-direction: column;
  gap: 0;
}
.detailTrackHeading {
  margin-bottom: 10px;
}
.playlistArtwork {
  width: 100%;
  height: 100%;
  display: grid;
}
.playlistArtwork.collage {
  grid-template-columns: repeat(2, minmax(0, 1fr));
  grid-template-rows: repeat(2, minmax(0, 1fr));
}
.playlistArtwork :deep(img) {
  min-width: 0;
  min-height: 0;
}
.emptyPlaylist {
  padding: 24px 0;
  color: var(--text-subdued);
}
#editPlaylistNameInput {
  width: 100%;
  padding: 12px;
  border: 1px solid var(--surface-border);
  border-radius: 4px;
  background: var(--surface-raised);
  color: var(--text-base);
  font: inherit;
}
</style>
