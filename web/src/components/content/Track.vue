<template>
  <div v-if="track" class="trackPage">
    <div class="topSection">
      <MultiSourceImage class="coverImage" :urls="coverUrls" />
      <div class="trackInfoColumn">
        <div class="trackIdentity">
          <p class="eyebrow">Track</p>
          <h1 class="trackName">{{ track.name }}</h1>
          <p v-if="album" class="albumLine">
            From album
            <button
              type="button"
              class="albumLink"
              @click.stop="handleClickOnAlbumName"
            >
              {{ album.name }}<span v-if="albumYear"> ({{ albumYear }})</span>
            </button>
          </p>
          <p v-if="trackMetaSummary" class="trackMetaSummary">
            {{ trackMetaSummary }}
          </p>
          <div v-if="trackBadges.length" class="trackBadges">
            <span v-for="badge in trackBadges" :key="badge">{{ badge }}</span>
          </div>
          <div v-if="trackSummary" class="trackSummaryBlock">
            <p
              ref="summaryTextRef"
              class="trackSummaryText"
              :class="{ expanded: summaryExpanded }"
            >
              {{ trackSummary }}
            </p>
            <button
              v-if="summaryOverflows"
              type="button"
              class="trackSummaryToggle"
              @click="summaryExpanded = !summaryExpanded"
            >
              {{ summaryExpanded ? "Show less" : "Read more" }}
            </button>
          </div>
        </div>
        <div class="trackActions">
          <PlayIcon
            class="playTrackIcon scaleClickFeedback bigIcon"
            @click.stop="handleClickOnPlayTrack"
          />
          <RadioIcon
            class="radioIcon scaleClickFeedback"
            title="Listen to radio"
            @click.stop="handleClickOnTrackRadio"
          />
          <button
            class="secondaryActionButton"
            type="button"
            @click.stop="showRadioBuilder = true"
          >
            Customize radio
          </button>
          <button
            v-if="showDownloadButton"
            class="secondaryActionButton"
            type="button"
            :disabled="isRequestingDownload"
            @click.stop="handleRequestDownload"
          >
            {{ isRequestingDownload ? "Requesting..." : "Request download" }}
          </button>
          <ToggableFavoriteIcon
            :toggled="isTrackLiked"
            :clickCallback="handleClickOnFavoriteIcon"
          />
        </div>
        <p v-if="downloadRequestMessage" class="downloadRequestMessage">
          {{ downloadRequestMessage }}
        </p>
      </div>
      <EnrichmentStatusIndicator
        :status="track.enrichment_status"
        entityType="track"
      />
    </div>

    <section v-if="detailRows.length" class="detailSection">
      <div v-for="row in detailRows" :key="row.label" class="detailItem">
        <dt>{{ row.label }}</dt>
        <dd>{{ row.value }}</dd>
      </div>
    </section>

    <section
      v-if="trackTags.length || trackContributors.length"
      class="metadataSection"
    >
      <div v-if="trackTags.length" class="metadataGroup">
        <h2>Tags</h2>
        <div class="tagList">
          <span v-for="tag in trackTags" :key="`${tag.tag_type}-${tag.tag}`">
            {{ tag.tag }}
          </span>
        </div>
      </div>
      <div v-if="trackContributors.length" class="metadataGroup">
        <h2>Credits</h2>
        <dl class="creditsList">
          <div
            v-for="contributor in trackContributors"
            :key="`${contributor.role}-${contributor.contributor_name}`"
          >
            <dt>{{ titleCase(contributor.role) }}</dt>
            <dd>{{ contributor.contributor_name }}</dd>
          </div>
        </dl>
      </div>
    </section>

    <section v-if="artistIds.length" class="artistsSection">
      <h2>Artists</h2>
      <div class="artistsContainer">
        <LoadArtistListItem
          v-for="artistId in artistIds"
          :key="artistId"
          :artistId="artistId"
        />
      </div>
    </section>

    <RadioBuilderModal
      :isOpen="showRadioBuilder"
      seedEntityType="track"
      :seedEntityId="trackId"
      @close="showRadioBuilder = false"
    />
  </div>
  <div v-else>
    <p>Loading {{ trackId }}...</p>
  </div>
</template>

<script setup>
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import MultiSourceImage from "@/components/common/MultiSourceImage.vue";
import PlayIcon from "@/components/icons/PlayIcon.vue";
import RadioIcon from "@/components/icons/RadioIcon.vue";
import { usePlaybackStore } from "@/store/playback";
import { useRemoteStore } from "@/store/remote";
import { chooseAlbumCoverImageUrl, formatDuration } from "@/utils";
import { useRouter } from "vue-router";
import LoadArtistListItem from "@/components/common/LoadArtistListItem.vue";
import { useStaticsStore } from "@/store/statics";
import { useUserStore } from "@/store/user";
import ToggableFavoriteIcon from "@/components/common/ToggableFavoriteIcon.vue";
import EnrichmentStatusIndicator from "@/components/common/EnrichmentStatusIndicator.vue";
import RadioBuilderModal from "@/components/common/RadioBuilderModal.vue";

const props = defineProps({
  trackId: {
    type: String,
    required: true,
  },
});

const track = ref(null);
const album = ref(null);
const coverUrls = ref([]);
const isTrackLiked = ref(false);
const showRadioBuilder = ref(false);
const isRequestingDownload = ref(false);
const downloadRequestMessage = ref(null);
const summaryTextRef = ref(null);
const summaryExpanded = ref(false);
const summaryOverflows = ref(false);

const router = useRouter();
const playback = usePlaybackStore();
const staticsStore = useStaticsStore();
const remoteStore = useRemoteStore();
const userStore = useUserStore();

const trackEnrichment = computed(() => track.value?.enrichment || null);
const trackProfile = computed(() => trackEnrichment.value?.profile || null);
const artistIds = computed(() => track.value?.artists_ids || []);

const titleCase = (value) => {
  if (!value) return null;
  return String(value)
    .replace(/_/g, " ")
    .replace(/\b\w/g, (char) => char.toUpperCase());
};

const extractYear = (value) => {
  if (!value) return null;
  if (typeof value === "number" && Number.isFinite(value)) {
    const year = new Date(value * 1000).getFullYear();
    return Number.isFinite(year) ? String(year) : null;
  }
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

const joinParts = (parts) => parts.filter(Boolean).join(" / ");

const albumYear = computed(() =>
  extractYear(album.value?.release_date || album.value?.date),
);

const trackSummary = computed(() => {
  const profile = trackProfile.value;
  return (
    profile?.summary || profile?.notes || profile?.performance_context || null
  );
});

const trackMetaSummary = computed(() => {
  const profile = trackProfile.value;
  return joinParts([
    formatDuration(track.value?.duration || 0),
    titleCase(profile?.track_kind),
    profile?.recording_date
      ? `Recorded ${formatEnrichmentDate(profile.recording_date)}`
      : null,
    profile?.composition_date
      ? `Composed ${formatEnrichmentDate(profile.composition_date)}`
      : null,
    profile?.language || track.value?.language,
  ]);
});

const trackBadges = computed(() => {
  const profile = trackProfile.value || {};
  return [
    [track.value?.explicit || track.value?.is_explicit, "Explicit"],
    [!isTrackAvailable.value, "Unavailable"],
    [profile.is_instrumental, "Instrumental"],
    [profile.is_live, "Live"],
    [profile.is_cover, "Cover"],
    [profile.is_remix, "Remix"],
    [profile.is_remaster, "Remaster"],
    [profile.is_arrangement, "Arrangement"],
  ]
    .filter(([enabled]) => enabled)
    .map(([, label]) => label);
});

const movementLabel = computed(() => {
  const profile = trackProfile.value;
  if (!profile) return null;
  return joinParts([
    profile.movement_number ? `No. ${profile.movement_number}` : null,
    profile.movement_title,
  ]);
});

const detailRows = computed(() => {
  const profile = trackProfile.value || {};
  return [
    { label: "Work", value: profile.work_title },
    { label: "Movement", value: movementLabel.value },
    { label: "Form", value: titleCase(profile.form) },
    { label: "Key", value: profile.key_signature },
    { label: "Opus", value: profile.opus_number },
    { label: "Catalog", value: profile.catalog_number },
    { label: "ISRC", value: track.value?.external_id_isrc },
    { label: "Track", value: track.value?.track_number },
    { label: "Disc", value: track.value?.disc_number },
  ].filter(
    (row) => row.value !== null && row.value !== undefined && row.value !== "",
  );
});

const trackTags = computed(() => trackEnrichment.value?.tags || []);
const trackContributors = computed(() =>
  (trackEnrichment.value?.contributors || []).slice(0, 12),
);

const isTrackAvailable = computed(() => {
  const availability = track.value?.availability;
  return !availability || availability === "available";
});

const isTrackFetching = computed(
  () => track.value?.availability === "fetching",
);
const showDownloadButton = computed(
  () =>
    userStore.canRequestContent &&
    !isTrackAvailable.value &&
    !isTrackFetching.value,
);

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

watch(
  trackSummary,
  () => {
    summaryExpanded.value = false;
    updateSummaryOverflow();
  },
  { immediate: true },
);

watch(
  [() => userStore.likedTrackIds, track],
  ([likedTracks, trackData]) => {
    isTrackLiked.value = Boolean(
      likedTracks && trackData && likedTracks.includes(props.trackId),
    );
  },
  { immediate: true },
);

let trackDataUnwatcher = null;
let albumDataUnwatcher = null;

const stopWatchers = () => {
  if (trackDataUnwatcher) {
    trackDataUnwatcher();
    trackDataUnwatcher = null;
  }
  if (albumDataUnwatcher) {
    albumDataUnwatcher();
    albumDataUnwatcher = null;
  }
};

const handleClickOnPlayTrack = () => {
  if (!track.value || !isTrackAvailable.value) return;
  playback.setTrack({
    id: track.value.id,
    name: track.value.name,
    artists: track.value.artists_ids,
    duration: track.value.duration,
    album_id: track.value.album_id,
  });
};

const handleClickOnTrackRadio = () => {
  playback.setRadioFromItem("track", props.trackId);
};

const handleClickOnFavoriteIcon = () => {
  userStore.setTrackIsLiked(props.trackId, !isTrackLiked.value);
};

const handleClickOnAlbumName = () => {
  if (album.value?.id) {
    router.push("/album/" + album.value.id);
  }
};

const handleRequestDownload = async () => {
  if (isRequestingDownload.value) return;

  isRequestingDownload.value = true;
  downloadRequestMessage.value = null;
  try {
    const result = await remoteStore.requestTrackDownload(props.trackId);
    downloadRequestMessage.value = result.success
      ? "Download queued"
      : result.error || "Failed to request download";
  } finally {
    isRequestingDownload.value = false;
  }
};

const fetchTrack = async (id) => {
  stopWatchers();
  track.value = null;
  album.value = null;
  coverUrls.value = [];
  downloadRequestMessage.value = null;

  if (!id) return;

  trackDataUnwatcher = watch(
    staticsStore.getTrack(id),
    (newData) => {
      if (newData && newData.item && typeof newData.item === "object") {
        track.value = newData.item;
        if (albumDataUnwatcher) {
          albumDataUnwatcher();
          albumDataUnwatcher = null;
        }
        albumDataUnwatcher = watch(
          staticsStore.getAlbum(newData.item.album_id),
          (newAlbumData) => {
            if (
              newAlbumData &&
              newAlbumData.item &&
              typeof newAlbumData.item === "object"
            ) {
              album.value = newAlbumData.item;
              coverUrls.value = chooseAlbumCoverImageUrl(newAlbumData.item);
            }
          },
          { immediate: true },
        );
      }
    },
    { immediate: true },
  );
};

watch(
  () => props.trackId,
  (newId) => {
    fetchTrack(newId);
    if (newId) {
      remoteStore.recordImpression("track", newId);
    }
  },
);

onMounted(() => {
  fetchTrack(props.trackId);
  remoteStore.recordImpression("track", props.trackId);
  updateSummaryOverflow();
  window.addEventListener("resize", updateSummaryOverflow);
});

onUnmounted(() => {
  stopWatchers();
  window.removeEventListener("resize", updateSummaryOverflow);
});
</script>

<style scoped>
@import "@/assets/icons.css";

.trackPage {
  color: var(--text-base);
}

.topSection {
  position: relative;
  display: grid;
  grid-template-columns: minmax(180px, 300px) minmax(0, 1fr);
  gap: clamp(20px, 3vw, 36px);
  align-items: start;
  padding: clamp(18px, 3vw, 32px);
  border: 1px solid var(--surface-border);
  border-radius: 8px;
  background: linear-gradient(
      135deg,
      rgba(125, 99, 255, 0.16),
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

.trackInfoColumn {
  min-width: 0;
  align-self: stretch;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  justify-content: space-between;
  gap: 22px;
}

.trackIdentity {
  min-width: 0;
}

.eyebrow {
  margin: 0 0 8px;
  color: var(--text-subdued);
  font-size: 0.78rem;
  font-weight: 850;
  letter-spacing: 0;
  text-transform: uppercase;
}

.trackName {
  margin: 0;
  color: var(--text-base);
  font-size: clamp(2rem, 4.6vw, 4.6rem);
  font-weight: 900;
  line-height: 0.96;
  letter-spacing: 0;
  overflow-wrap: anywhere;
}

.albumLine,
.trackMetaSummary {
  margin: 12px 0 0;
  color: var(--text-muted);
  font-size: clamp(0.95rem, 1.3vw, 1.1rem);
  font-weight: 650;
  line-height: 1.35;
  overflow-wrap: anywhere;
}

.albumLink {
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--text-base);
  font: inherit;
  font-weight: 800;
  cursor: pointer;
}

.albumLink:hover {
  color: var(--spotify-green);
  text-decoration: underline;
}

.trackBadges,
.tagList {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.trackBadges {
  margin-top: 14px;
}

.trackBadges span,
.tagList span {
  min-height: 26px;
  padding: 4px 9px;
  border: 1px solid var(--surface-border);
  border-radius: 999px;
  color: var(--text-muted);
  background: rgba(255, 255, 255, 0.04);
  font-size: 0.8rem;
  font-weight: 750;
  line-height: 1.2;
}

.trackSummaryBlock {
  max-width: 860px;
  margin-top: 18px;
}

.trackSummaryText {
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 3;
  margin: 0;
  overflow: hidden;
  color: var(--text-muted);
  font-size: 0.96rem;
  line-height: 1.5;
}

.trackSummaryText.expanded {
  display: block;
  -webkit-line-clamp: unset;
  overflow: visible;
}

.trackSummaryToggle {
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

.trackSummaryToggle:hover {
  color: var(--spotify-green);
}

.trackActions {
  display: flex;
  margin-top: auto;
  align-items: center;
  flex-wrap: wrap;
  gap: 12px;
}

.trackActions :deep(.bigIcon) {
  width: 42px;
  height: 42px;
}

.playTrackIcon {
  width: 54px;
  height: 54px;
  fill: var(--spotify-green);
  cursor: pointer;
}

.radioIcon {
  width: 42px;
  height: 42px;
  cursor: pointer;
  color: var(--spotify-green);
}

.radioIcon:hover {
  color: var(--spotify-green-hover);
}

.secondaryActionButton {
  width: fit-content;
  min-height: 38px;
  padding: 0 16px;
  border: 1px solid var(--surface-border-strong);
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.04);
  color: var(--text-base);
  font-size: 0.86rem;
  font-weight: 800;
  cursor: pointer;
}

.secondaryActionButton:hover:not(:disabled) {
  background: var(--surface-hover);
}

.secondaryActionButton:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.downloadRequestMessage {
  margin: -12px 0 0;
  color: var(--text-subdued);
  font-size: 0.9rem;
}

.detailSection {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
  gap: 1px;
  margin: 18px 0 0;
  border: 1px solid var(--surface-border);
  border-radius: 8px;
  overflow: hidden;
  background: var(--surface-border);
}

.detailItem {
  min-width: 0;
  padding: 13px 14px;
  background: var(--surface-panel);
}

.detailItem dt,
.creditsList dt {
  color: var(--text-subdued);
  font-size: 0.76rem;
  font-weight: 850;
  text-transform: uppercase;
}

.detailItem dd,
.creditsList dd {
  margin: 4px 0 0;
  color: var(--text-base);
  font-size: 0.95rem;
  font-weight: 650;
  overflow-wrap: anywhere;
}

.metadataSection {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
  gap: 18px;
  margin: 18px 0 0;
}

.metadataGroup h2,
.artistsSection h2 {
  margin: 0 0 10px;
  color: var(--text-base);
  font-size: 1rem;
  font-weight: 850;
}

.creditsList {
  display: grid;
  gap: 8px;
  margin: 0;
}

.creditsList div {
  padding: 10px 0;
  border-top: 1px solid var(--surface-border);
}

.artistsSection {
  margin-top: 18px;
}

.artistsContainer {
  width: 100%;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: 8px;
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
