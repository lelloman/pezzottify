<template>
  <DetailPage
    v-if="album"
    class="albumDetail"
    :title="album.name"
    kind="Album"
    @artwork-contextmenu.prevent="
      entityMenu?.openMenu($event, 'album', albumId, album.name)
    "
    :imageUrls="coverUrls || []"
  >
    <template #meta>
      <div class="albumHeaderMeta">
        <LoadClickableArtistsNames
          v-if="album.artists_ids?.length"
          class="albumHeaderArtists"
          :artistsIds="album.artists_ids"
        />
        <span v-if="albumReleaseYear" class="albumMetaPart">{{
          albumReleaseYear
        }}</span>
        <span class="albumMetaPart"
          >{{ albumTrackCount }} {{ albumTrackCount === 1 ? "track" : "tracks"
          }}<span v-if="albumDuration">, {{ albumDurationLabel }}</span></span
        >
      </div>
    </template>
    <template #actions>
      <DetailActions
        playLabel="Play album"
        showSave
        :saved="isAlbumLiked"
        @play="handleClickOnPlayAlbum"
        @save="handleClickOnFavoriteIcon"
      >
        <template #inline>
          <DownloadAction
            v-if="
              showDownloadSection ||
              (userStore.canRequestContent &&
                downloadRequestState !== 'can_request')
            "
            :state="downloadRequestState"
            :busy="isRequesting"
            :progress="downloadProgress"
            :queuePosition="effectiveQueuePosition"
            :error="downloadError"
            @request="handleRequestDownload"
          />
          <RadioAction
            @start="handleClickOnAlbumRadio"
            @customize="showRadioBuilder = true"
          />
        </template>
        <template #secondary>
          <button
            class="advancedRadioButton steerButton"
            title="Steer the current queue toward this album"
            @click.stop="showDestinationPrompt = true"
          >
            <SteeringWheelIcon class="steerButtonIcon" />
            <span class="actionLabel">Steer here</span>
          </button>
          <button
            v-if="canAddToDestinationMix"
            class="advancedRadioButton steerButton"
            title="Add this album to the current destination mix"
            @click.stop="
              playback.addGravityDestinationComponent({
                entity_type: 'album',
                entity_id: albumId,
                label: album.name,
              })
            "
          >
            <PlaylistPlusIcon class="steerButtonIcon" />
            <span class="actionLabel">Add to mix</span>
          </button>
        </template>
      </DetailActions>
    </template>

    <div class="tracksContainer">
      <div class="detailTrackHeading">
        <span>#</span><span>Title</span><span>Duration</span>
      </div>
      <div
        v-for="(disc, discIndex) in album.discs"
        :key="disc"
        class="discContainer"
      >
        <h2 v-if="album.discs.length > 1">
          Disc {{ discIndex + 1
          }}<span v-if="disc.name">- {{ disc.name }}</span>
        </h2>
        <div
          v-for="(trackId, trackIndex) in disc.tracks"
          :key="trackId"
          class="track"
          @contextmenu.prevent="
            openTrackContextMenu($event, trackId, trackIndex)
          "
        >
          <LoadTrackListItem
            albumLayout
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
    <section
      v-if="
        albumSummary ||
        albumExtraDetails ||
        albumBadges.length ||
        ['queued', 'running', 'failed', 'failed_enrichment'].includes(
          album.enrichment_status?.status,
        )
      "
      class="detailSupporting"
    >
      <h2 class="detailSectionTitle">About this album</h2>
      <p v-if="albumExtraDetails" class="albumExtraDetails">
        {{ albumExtraDetails }}
      </p>
      <div v-if="albumSummary" class="albumSummaryBlock">
        <p
          ref="summaryTextRef"
          class="albumSummaryText"
          :class="{ expanded: summaryExpanded }"
        >
          {{ albumSummary }}
        </p>
        <button
          v-if="summaryOverflows"
          type="button"
          class="albumSummaryToggle"
          @click="summaryExpanded = !summaryExpanded"
        >
          {{ summaryExpanded ? "Show less" : "Read more" }}
        </button>
      </div>
      <div v-if="albumBadges.length" class="albumBadges">
        <span v-for="badge in albumBadges" :key="badge">{{ badge }}</span>
      </div>
      <EnrichmentStatusIndicator
        :status="album.enrichment_status"
        entityType="album"
      />
    </section>
    <section class="detailSupporting">
      <h2 class="detailSectionTitle">Artists</h2>
      <div class="artistsContainer">
        <LoadArtistListItem
          v-for="artistId in album.artists_ids"
          :key="artistId"
          :artistId="artistId"
        />
      </div>
    </section>
    <TrackContextMenu ref="trackContextMenuRef" />
    <DestinationStepsPrompt
      :isOpen="showDestinationPrompt"
      :reference="{
        entity_type: 'album',
        entity_id: albumId,
        label: album.name,
      }"
      @close="showDestinationPrompt = false"
    />
    <EntityContextMenu ref="entityMenu" />
    <RadioBuilderModal
      :isOpen="showRadioBuilder"
      seedEntityType="album"
      :seedEntityId="albumId"
      @close="showRadioBuilder = false"
    />
  </DetailPage>
  <div v-else>
    <p>Loading {{ albumId }}...</p>
  </div>
</template>

<script setup>
import DownloadAction from "@/components/common/DownloadAction.vue";
import RadioAction from "@/components/common/RadioAction.vue";
import PlaylistPlusIcon from "@/components/icons/PlaylistPlusIcon.vue";
import DetailPage from "@/components/common/DetailPage.vue";
import DetailActions from "@/components/common/DetailActions.vue";
import LoadClickableArtistsNames from "@/components/common/LoadClickableArtistsNames.vue";
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { MAX_COMPONENTS } from "@/utils/gravity";
import { chooseAlbumCoverImageUrl } from "@/utils";
import { canRequestAlbumDownload } from "@/utils/downloadRequests";
import { usePlaybackStore } from "@/store/playback";
import { useUserStore } from "@/store/user";
import { useRemoteStore } from "@/store/remote";
import LoadArtistListItem from "@/components/common/LoadArtistListItem.vue";
import TrackContextMenu from "@/components/common/contextmenu/TrackContextMenu.vue";
import LoadTrackListItem from "../common/LoadTrackListItem.vue";
import { useStaticsStore } from "@/store/statics";
import RadioBuilderModal from "@/components/common/RadioBuilderModal.vue";
import DestinationStepsPrompt from "@/components/common/DestinationStepsPrompt.vue";
import EntityContextMenu from "@/components/common/contextmenu/EntityContextMenu.vue";
import SteeringWheelIcon from "@/components/icons/SteeringWheelIcon.vue";
import EnrichmentStatusIndicator from "@/components/common/EnrichmentStatusIndicator.vue";

const props = defineProps({
  albumId: {
    type: String,
    required: true,
  },
});

const album = ref(null);
const albumTrackIds = computed(
  () => album.value?.discs?.flatMap((d) => d.tracks) || [],
);
const albumTrackCount = computed(() => albumTrackIds.value.length);
const albumDuration = computed(() => {
  const durations = albumTrackIds.value.map(
    (id) => staticsStore.getTrack(id).item?.duration,
  );
  return durations.length && durations.every((d) => Number.isFinite(d))
    ? Math.round(durations.reduce((a, b) => a + b, 0) / 60000)
    : null;
});
const albumDurationLabel = computed(() => {
  const minutes = albumDuration.value;
  return minutes >= 60
    ? `${Math.floor(minutes / 60)} hr ${minutes % 60} min`
    : `${minutes} min`;
});
const coverUrls = ref(null);

const playback = usePlaybackStore();
const canAddToDestinationMix = computed(() => {
  const destination = playback.currentGravity?.destination;
  return Boolean(destination?.length && destination.length < MAX_COMPONENTS);
});
const userStore = useUserStore();
const staticsStore = useStaticsStore();
const remoteStore = useRemoteStore();
const summaryTextRef = ref(null);
const summaryExpanded = ref(false);
const summaryOverflows = ref(false);

const albumEnrichment = computed(() => album.value?.enrichment || null);
const albumProfile = computed(() => albumEnrichment.value?.profile || null);

const albumSummary = computed(() => {
  const profile = albumProfile.value;
  return profile?.summary || profile?.notes || null;
});

const updateSummaryOverflow = async () => {
  await nextTick();

  const element = summaryTextRef.value;
  if (!element) {
    summaryOverflows.value = false;
    return;
  }

  const styles = window.getComputedStyle(element);
  const lineHeight = Number.parseFloat(styles.lineHeight);
  const maxCollapsedHeight = Number.isFinite(lineHeight) ? lineHeight * 3 : 0;
  summaryOverflows.value =
    maxCollapsedHeight > 0 && element.scrollHeight > maxCollapsedHeight + 1;
};

const extractYear = (value) => {
  if (!value) return null;
  const match = String(value).match(/^(\d{4})/);
  return match ? match[1] : null;
};

const formatEnrichmentDate = (value) => {
  if (!value) return null;

  const match = String(value).match(/^(\d{4})(?:-(\d{2})(?:-(\d{2}))?)?$/);
  if (!match) return String(value);

  const [, year, month, day] = match;
  if (!month) return year;

  const date = new Date(
    Date.UTC(Number(year), Number(month) - 1, Number(day || 1)),
  );
  if (Number.isNaN(date.getTime())) return String(value);

  const options = day
    ? { month: "long", day: "numeric", year: "numeric", timeZone: "UTC" }
    : { month: "long", year: "numeric", timeZone: "UTC" };

  return new Intl.DateTimeFormat(undefined, options).format(date);
};

const titleCase = (value) => {
  if (!value) return null;
  return String(value)
    .replace(/_/g, " ")
    .replace(/\b\w/g, (char) => char.toUpperCase());
};

const formatDateRange = (start, end) => {
  const formattedStart = formatEnrichmentDate(start);
  const formattedEnd = formatEnrichmentDate(end);
  if (formattedStart && formattedEnd && formattedStart !== formattedEnd) {
    return `${formattedStart} - ${formattedEnd}`;
  }
  return formattedStart || formattedEnd;
};

const albumReleaseYear = computed(
  () =>
    extractYear(albumProfile.value?.original_release_date) ||
    extractYear(album.value?.release_date),
);
const albumExtraDetails = computed(() => {
  const profile = albumProfile.value;
  const recordingRange = formatDateRange(
    profile?.recording_start_date,
    profile?.recording_end_date,
  );
  const label = [profile?.label, profile?.catalog_number]
    .filter(Boolean)
    .join(" ");

  return [
    recordingRange ? `Recorded ${recordingRange}` : null,
    profile?.release_country,
    label || null,
    titleCase(profile?.album_kind),
  ]
    .filter(Boolean)
    .join(" • ");
});

const albumBadges = computed(() => {
  const profile = albumProfile.value;
  if (!profile) return [];

  return [
    [profile.is_live, "Live"],
    [profile.is_compilation, "Compilation"],
    [profile.is_soundtrack, "Soundtrack"],
    [profile.is_concept_album, "Concept album"],
    [profile.is_remix_album, "Remix album"],
    [profile.is_archival, "Archival"],
  ]
    .filter(([enabled]) => enabled)
    .map(([, label]) => label);
});

watch(
  albumSummary,
  () => {
    summaryExpanded.value = false;
    updateSummaryOverflow();
  },
  { immediate: true },
);

const currentTrackId = ref(null);
const currentTrackIndex = ref(null);
const isAlbumLiked = ref(false);
const showRadioBuilder = ref(false);
const showDestinationPrompt = ref(false);
const entityMenu = ref(null);

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

const showDownloadSection = computed(() =>
  canRequestAlbumDownload(userStore.canRequestContent, albumAvailability.value),
);

const syncedDownloadRequest = computed(() => {
  return userStore.getDownloadRequest(props.albumId);
});

const downloadProgress = computed(() => {
  return (
    syncedDownloadRequest.value?.progress ||
    existingRequest.value?.progress ||
    null
  );
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
      if (artistRef.item?.name) {
        artistName = artistRef.item.name;
      }
    }

    const result = await remoteStore.requestAlbumDownload(
      props.albumId,
      album.value.name,
      artistName,
    );

    if (result.success) {
      existingRequest.value = { status: "pending", ...result.data };
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
  updateSummaryOverflow();
  window.addEventListener("resize", updateSummaryOverflow);
});

onUnmounted(() => {
  window.removeEventListener("resize", updateSummaryOverflow);
});
</script>

<style scoped>
.discContainer + .discContainer {
  margin-top: 24px;
}

.albumDetail :deep(.detailIdentity h1:not(.longTitle)) {
  font-size: clamp(2rem, 5.5cqw, 4rem);
}
.albumHeaderMeta {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 4px 0;
  color: rgba(255, 255, 255, 0.7);
  font-size: 14px;
  font-weight: 400;
}
.albumHeaderArtists {
  color: #fff;
  font-weight: 700;
}
.albumMetaPart:not(:first-child)::before {
  content: "•";
  margin: 0 6px;
}
.albumExtraDetails {
  color: var(--text-subdued);
  font-size: var(--text-sm);
  line-height: 1.5;
}
@container (max-width:560px) {
  .albumDetail :deep(.detailIdentity h1:not(.longTitle)) {
    font-size: clamp(2rem, 8cqw, 3rem);
  }
}

.albumBadges {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 14px;
}

.albumBadges span {
  min-height: 26px;
  padding: 4px 9px;
  border: 1px solid var(--surface-border);
  border-radius: 999px;
  color: var(--text-muted);
  background: rgba(255, 255, 255, 0.04);
  font-size: var(--text-sm);
  font-weight: 750;
  line-height: 1.2;
}

.albumSummaryBlock {
  max-width: 860px;
  margin-top: 18px;
}

.albumSummaryText {
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 3;
  margin: 0;
  overflow: hidden;
  color: var(--text-muted);
  font-size: 0.96rem;
  line-height: 1.5;
}

.albumSummaryText.expanded {
  display: block;
  -webkit-line-clamp: unset;
  overflow: visible;
}

.albumSummaryToggle {
  margin: 6px 0 0;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--text-base);
  font: inherit;
  font-size: 0.9rem;
  font-weight: 800;
  cursor: pointer;
}

.albumSummaryToggle:hover {
  color: var(--spotify-green);
}

.advancedRadioButton {
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

.advancedRadioButton {
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
  padding-top: 0;
}

.discContainer h2 {
  margin: 0 0 8px;
  color: var(--text-subdued);
  font-size: 0.82rem;
  font-weight: 850;
  text-transform: uppercase;
}

.steerButton {
  display: inline-flex;
  align-items: center;
  gap: 8px;
}

.steerButtonIcon {
  width: 16px;
  height: 16px;
  fill: currentColor;
}
</style>
