<template>
  <div v-if="album">
    <div class="topSection">
      <MultiSourceImage class="coverImage" :urls="coverUrls" />
      <div class="albumInfoColum">
        <h1 class="albumName">{{ album.name }}</h1>
      </div>
    </div>
    <div class="commandsSection">
      <PlayIcon
        class="playAlbumIcon scaleClickFeedback bigIcon"
        @click.stop="handleClickOnPlayAlbum"
      />
      <RadioIcon
        class="playAlbumIcon scaleClickFeedback bigIcon radioIcon"
        title="Listen to radio"
        @click.stop="handleClickOnAlbumRadio"
      />
      <button class="advancedRadioButton" @click.stop="showRadioBuilder = true">
        Customize radio
      </button>
      <ToggableFavoriteIcon
        :toggled="isAlbumLiked"
        :clickCallback="handleClickOnFavoriteIcon"
      />
    </div>

    <!-- Download Request Section -->
    <div v-if="showDownloadSection" class="downloadRequestSection">
      <div
        v-if="downloadRequestState === 'can_request'"
        class="downloadRequestContent"
      >
        <button
          class="downloadRequestButton"
          @click="handleRequestDownload"
          :disabled="isRequesting"
        >
          <span v-if="isRequesting">Requesting...</span>
          <span v-else>Request Download</span>
        </button>
      </div>
      <div
        v-else-if="downloadRequestState === 'pending'"
        class="downloadRequestContent statusPending"
      >
        <span class="statusIcon">⏳</span>
        <span
          >Download queued{{
            effectiveQueuePosition ? ` (#${effectiveQueuePosition})` : ""
          }}</span
        >
      </div>
      <div
        v-else-if="downloadRequestState === 'in_progress'"
        class="downloadRequestContent statusInProgress"
      >
        <span class="statusIcon">⬇️</span>
        <span v-if="downloadProgress"
          >Downloading {{ downloadProgress.completed }}/{{
            downloadProgress.total_children
          }}
          tracks</span
        >
        <span v-else>Downloading...</span>
      </div>
      <div
        v-else-if="downloadRequestState === 'completed'"
        class="downloadRequestContent statusCompleted"
      >
        <span class="statusIcon">✅</span>
        <span>Download completed</span>
      </div>
      <div
        v-else-if="downloadRequestState === 'failed'"
        class="downloadRequestContent statusFailed"
      >
        <span class="statusIcon">❌</span>
        <span>Download failed</span>
        <button class="retryButton" @click="handleRequestDownload">
          Retry
        </button>
      </div>
      <div
        v-else-if="downloadRequestState === 'error'"
        class="downloadRequestContent statusFailed"
      >
        <span class="statusIcon">❌</span>
        <span>{{ downloadError || "Failed to request" }}</span>
        <button class="retryButton" @click="handleRequestDownload">
          Retry
        </button>
      </div>
    </div>

    <div class="artistsContainer">
      <LoadArtistListItem
        v-for="artistId in album.artists_ids"
        :key="artistId"
        :artistId="artistId"
      />
    </div>
    <div class="tracksContainer">
      <div
        v-for="(disc, discIndex) in album.discs"
        :key="disc"
        class="discContainer"
      >
        <h1 v-if="album.discs.length > 1">
          Disc {{ discIndex + 1
          }}<span v-if="disc.name">- {{ disc.name }}</span>
        </h1>
        <div
          v-for="(trackId, trackIndex) in disc.tracks"
          :key="trackId"
          class="track"
          @contextmenu.prevent="
            openTrackContextMenu($event, trackId, trackIndex)
          "
        >
          <LoadTrackListItem
            :contextId="albumId"
            :trackId="trackId"
            :trackNumber="trackIndex + 1"
            @track-clicked="handleClickOnTrack(trackId)"
            :isCurrentlyPlaying="
              getFlatTrackIndex(discIndex, trackIndex) == currentTrackIndex
            "
          />
        </div>
      </div>
    </div>
    <TrackContextMenu ref="trackContextMenuRef" />
    <RadioBuilderModal
      :isOpen="showRadioBuilder"
      seedEntityType="album"
      :seedEntityId="albumId"
      @close="showRadioBuilder = false"
    />
  </div>
  <div v-else>
    <p>Loading {{ albumId }}...</p>
  </div>
</template>

<script setup>
import { ref, watch, onMounted, computed } from "vue";
import { chooseAlbumCoverImageUrl } from "@/utils";
import MultiSourceImage from "@/components/common/MultiSourceImage.vue";
import PlayIcon from "@/components/icons/PlayIcon.vue";
import RadioIcon from "@/components/icons/RadioIcon.vue";
import { usePlaybackStore } from "@/store/playback";
import { useUserStore } from "@/store/user";
import { useRemoteStore } from "@/store/remote";
import ToggableFavoriteIcon from "@/components/common/ToggableFavoriteIcon.vue";
import LoadArtistListItem from "@/components/common/LoadArtistListItem.vue";
import TrackContextMenu from "@/components/common/contextmenu/TrackContextMenu.vue";
import LoadTrackListItem from "../common/LoadTrackListItem.vue";
import { useStaticsStore } from "@/store/statics";
import RadioBuilderModal from "@/components/common/RadioBuilderModal.vue";

const props = defineProps({
  albumId: {
    type: String,
    required: true,
  },
});

const album = ref(null);
const coverUrls = ref(null);

const playback = usePlaybackStore();
const userStore = useUserStore();
const staticsStore = useStaticsStore();
const remoteStore = useRemoteStore();

const currentTrackId = ref(null);
const currentTrackIndex = ref(null);
const isAlbumLiked = ref(false);
const showRadioBuilder = ref(false);

// Download request state
const isRequesting = ref(false);
const downloadError = ref(null);
const existingRequest = ref(null);
const queuePosition = ref(null);

let albumDataUnwatcher = null;

const trackContextMenuRef = ref(null);
const openTrackContextMenu = (event, trackId, index) => {
  trackContextMenuRef.value.openMenu(event, trackId, index);
};

// Compute flat track index across all discs
const getFlatTrackIndex = (discIndex, trackIndex) => {
  if (!album.value || !album.value.discs) return -1;
  let flatIndex = trackIndex;
  for (let i = 0; i < discIndex; i++) {
    flatIndex += album.value.discs[i].tracks.length;
  }
  return flatIndex;
};

watch(
  () => playback.currentTrackId,
  (newTrackId) => {
    console.log("CurrentTrackId: " + newTrackId);
    currentTrackId.value = newTrackId;
  },
  { immediate: true },
);

watch(
  [() => playback.currentTrackIndex, () => playback.currentPlaylist],
  ([newTrackIndex, newPlaylist]) => {
    console.log(
      "Album.vue watcher - TrackIndex:",
      newTrackIndex,
      "Playlist:",
      newPlaylist,
      "AlbumId:",
      props.albumId,
    );
    if (
      newPlaylist &&
      newPlaylist.context &&
      newPlaylist.context.id === props.albumId &&
      Number.isInteger(newTrackIndex)
    ) {
      console.log("Album.vue - Setting currentTrackIndex to:", newTrackIndex);
      currentTrackIndex.value = newTrackIndex;
    } else {
      currentTrackIndex.value = null;
    }
  },
  { immediate: true },
);

const fetchData = async (id) => {
  if (albumDataUnwatcher) {
    albumDataUnwatcher();
    albumDataUnwatcher = null;
  }
  if (!id) return;

  albumDataUnwatcher = watch(
    staticsStore.getAlbum(id),
    (newData) => {
      if (newData && newData.item && typeof newData.item === "object") {
        coverUrls.value = chooseAlbumCoverImageUrl(newData.item);
        album.value = newData.item;
      }
    },
    { immediate: true },
  );
};

const handleClickOnFavoriteIcon = () => {
  userStore.setAlbumIsLiked(props.albumId, !isAlbumLiked.value);
};

const handleClickOnPlayAlbum = () => {
  playback.setAlbumId(props.albumId);
};

const handleClickOnAlbumRadio = () => {
  playback.setRadioFromItem("album", props.albumId);
};

const handleClickOnTrack = (trackId) => {
  if (trackId != currentTrackId.value) {
    const discIndex = album.value.discs.findIndex((disc) =>
      disc.tracks.includes(trackId),
    );
    const trackIndex = album.value.discs[discIndex].tracks.indexOf(trackId);
    playback.setAlbumId(props.albumId, discIndex, trackIndex);
  }
};

watch(
  album,
  (newAlbum) => {
    if (newAlbum) {
      coverUrls.value = chooseAlbumCoverImageUrl(newAlbum);
    }
  },
  { immediate: true },
);

watch(
  () => props.albumId,
  (newId) => {
    fetchData(newId);
    if (newId) {
      remoteStore.recordImpression("album", newId);
    }
  },
);

watch(
  () => userStore.likedAlbumIds,
  (likedAlbums) => {
    console.log(
      "watch liked albums and album data, new stuff incoming: " + likedAlbums,
    );
    if (likedAlbums) {
      isAlbumLiked.value = likedAlbums.includes(props.albumId);
      console.log("isAlbumLiked: " + isAlbumLiked.value);
      console.log("likedAlbums: " + likedAlbums);
    }
  },
  { immediate: true },
);

// Download request computed properties
const albumAvailability = computed(() => {
  const avail = album.value?.album_availability || "complete";
  console.log(
    "[Album] albumAvailability:",
    avail,
    "for album:",
    album.value?.name,
  );
  return avail;
});

const isAlbumUnavailable = computed(() => {
  return (
    albumAvailability.value === "missing" ||
    albumAvailability.value === "partial"
  );
});

const showDownloadSection = computed(() => {
  const canRequest = userStore.canRequestContent;
  const unavailable = isAlbumUnavailable.value;
  console.log(
    "[Album] showDownloadSection check - canRequestContent:",
    canRequest,
    "isAlbumUnavailable:",
    unavailable,
  );
  return canRequest && unavailable;
});

const syncedDownloadRequest = computed(() => {
  return userStore.getDownloadRequest(props.albumId);
});

const downloadProgress = computed(() => {
  return syncedDownloadRequest.value?.progress || null;
});

const effectiveQueuePosition = computed(() => {
  return syncedDownloadRequest.value?.queue_position || queuePosition.value;
});

const downloadRequestState = computed(() => {
  if (downloadError.value) return "error";

  // Prefer synced state (real-time) over API-fetched state
  const synced = syncedDownloadRequest.value;
  if (synced) {
    const status = synced.status?.toLowerCase();
    if (status === "pending") return "pending";
    if (status === "in_progress") return "in_progress";
    if (status === "completed") return "completed";
    if (status === "failed") return "failed";
  }

  if (!existingRequest.value) return "can_request";

  const status = existingRequest.value.status?.toLowerCase();
  if (status === "pending") return "pending";
  if (status === "in_progress") return "in_progress";
  if (status === "completed") return "completed";
  if (status === "failed") return "failed";
  return "can_request";
});

// Fetch existing download request for this album
const fetchDownloadRequest = async () => {
  if (!userStore.canRequestContent) return;

  try {
    const data = await remoteStore.fetchMyDownloadRequests();
    if (data) {
      const requests = data.requests || [];
      const request = requests.find((r) => r.content_id === props.albumId);
      if (request) {
        existingRequest.value = request;
        queuePosition.value = request.queue_position || null;
      } else {
        existingRequest.value = null;
        queuePosition.value = null;
      }
    }
  } catch (error) {
    console.error("Failed to fetch download requests:", error);
  }
};

// Request download handler
const handleRequestDownload = async () => {
  if (isRequesting.value || !album.value) return;

  isRequesting.value = true;
  downloadError.value = null;

  try {
    // Get artist name from first artist
    let artistName = "Unknown Artist";
    if (album.value.artists_ids && album.value.artists_ids.length > 0) {
      const artistRef = staticsStore.getArtist(album.value.artists_ids[0]);
      if (artistRef.value?.item?.name) {
        artistName = artistRef.value.item.name;
      }
    }

    const result = await remoteStore.requestAlbumDownload(
      props.albumId,
      album.value.name,
      artistName,
    );

    if (result.success) {
      // Refetch to get the new request status
      await fetchDownloadRequest();
    } else {
      downloadError.value = result.error || "Failed to request download";
    }
  } catch (error) {
    console.error("Failed to request download:", error);
    downloadError.value = "Failed to request download";
  } finally {
    isRequesting.value = false;
  }
};

// Watch for album changes to refetch download request
watch(
  () => props.albumId,
  () => {
    existingRequest.value = null;
    downloadError.value = null;
    fetchDownloadRequest();
  },
);

onMounted(() => {
  fetchData(props.albumId);
  remoteStore.recordImpression("album", props.albumId);
  fetchDownloadRequest();
});
</script>

<style scoped>
.topSection {
  display: grid;
  grid-template-columns: minmax(180px, 300px) minmax(0, 1fr);
  gap: clamp(20px, 3vw, 36px);
  align-items: end;
  padding: clamp(18px, 3vw, 32px);
  border: 1px solid var(--surface-border);
  border-radius: 8px;
  background: linear-gradient(
      135deg,
      rgba(29, 185, 84, 0.18),
      rgba(17, 20, 22, 0.58) 45%
    ),
    var(--surface-raised);
}

.coverImage {
  width: 100%;
  aspect-ratio: 1;
  height: auto;
  object-fit: cover;
  border-radius: 8px;
  box-shadow: var(--shadow-lg);
  overflow: hidden;
}

.albumInfoColum {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.albumName {
  margin: 0;
  color: var(--text-base);
  font-size: clamp(2rem, 4.6vw, 4.6rem);
  font-weight: 900;
  line-height: 0.96;
  letter-spacing: 0;
}

.playAlbumIcon {
  width: 54px;
  height: 54px;
  fill: var(--spotify-green);
}

.commandsSection {
  display: flex;
  flex-direction: row;
  align-items: center;
  flex-wrap: wrap;
  gap: 12px;
  margin: 18px 0;
}

.commandsSection > div {
  margin-left: 0;
}

.advancedRadioButton,
.downloadRequestButton,
.retryButton {
  min-height: 38px;
  padding: 0 16px;
  border-radius: 999px;
  font-size: 0.86rem;
  font-weight: 800;
  cursor: pointer;
  transition:
    background-color var(--transition-fast),
    border-color var(--transition-fast),
    opacity var(--transition-fast);
}

.advancedRadioButton,
.retryButton {
  border: 1px solid var(--surface-border-strong);
  background: rgba(255, 255, 255, 0.04);
  color: var(--text-base);
}

.advancedRadioButton:hover,
.retryButton:hover {
  background: var(--surface-hover);
  color: var(--text-base);
}

.artistsContainer {
  width: 100%;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: 8px;
  margin: 16px 0;
}

.tracksContainer {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.discContainer {
  border-top: 1px solid var(--surface-border);
  padding-top: 12px;
}

.discContainer h1 {
  margin: 0 0 8px;
  color: var(--text-subdued);
  font-size: 0.82rem;
  font-weight: 850;
  text-transform: uppercase;
}

.downloadRequestSection {
  margin: 16px 0;
  padding: 14px;
  background: var(--surface-raised);
  border: 1px solid var(--surface-border);
  border-radius: 8px;
}

.downloadRequestContent {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 12px;
}

.downloadRequestButton {
  background-color: var(--spotify-green);
  color: #071108;
  border: none;
}

.downloadRequestButton:hover:not(:disabled) {
  opacity: 0.9;
}

.downloadRequestButton:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.statusIcon {
  font-size: 18px;
}

.statusPending {
  color: var(--text-subdued);
}

.statusInProgress {
  color: var(--spotify-green);
}

.statusCompleted {
  color: #4caf50;
}

.statusFailed {
  color: #f44336;
}

@media (max-width: 720px) {
  .topSection {
    grid-template-columns: 1fr;
  }

  .coverImage {
    max-width: 280px;
  }
}
</style>
