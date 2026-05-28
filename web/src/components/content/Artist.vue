<template>
  <div v-if="artist">
    <div class="topSection">
      <MultiSourceImage class="coverImage" :urls="coverUrls" />
      <div class="artistInfoColum">
        <div class="artistIdentity">
          <h1 class="artistName">{{ artist.name }}</h1>
          <p v-if="lifeSummary" class="artistLifeSummary">{{ lifeSummary }}</p>
          <div v-if="shortBio" class="artistBioBlock">
            <p
              ref="bioTextRef"
              class="artistBioText"
              :class="{ expanded: bioExpanded }"
            >
              {{ shortBio }}
            </p>
            <button
              v-if="bioOverflows"
              type="button"
              class="artistBioToggle"
              @click="bioExpanded = !bioExpanded"
            >
              {{ bioExpanded ? "Show less" : "Read more" }}
            </button>
          </div>
        </div>
        <div class="artistActions">
          <ToggableFavoriteIcon
            :toggled="isArtistLiked"
            :clickCallback="handleClickOnFavoriteIcon"
          />
          <RadioIcon
            class="radioIcon scaleClickFeedback"
            title="Listen to radio"
            @click.stop="handleClickOnArtistRadio"
          />
          <button
            class="advancedRadioButton"
            @click.stop="showRadioBuilder = true"
          >
            Customize radio
          </button>
        </div>
      </div>
    </div>
    <div v-if="enrichmentLabel" class="enrichmentStatus">
      {{ enrichmentLabel }}
    </div>
    <div class="relatedArtistsContainer">
      <LoadArtistListItem
        v-for="artistId in artist.related"
        :key="artistId"
        :artistId="artistId"
      />
    </div>
    <div class="discographyContainer">
      <ArtistDiscography :artistId="artistId" />
    </div>
    <div class="discographyContainer">
      <ArtistDiscography :artistId="artistId" :appearsOn="true" />
    </div>
    <RadioBuilderModal
      :isOpen="showRadioBuilder"
      seedEntityType="artist"
      :seedEntityId="artistId"
      @close="showRadioBuilder = false"
    />
  </div>

  <div v-else>
    <p>Loading {{ artistId }}...</p>
  </div>
</template>

<script setup>
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { chooseArtistCoverImageUrl } from "@/utils";
import { useUserStore } from "@/store/user.js";
import { useStaticsStore } from "@/store/statics.js";
import { useRemoteStore } from "@/store/remote.js";
import MultiSourceImage from "@/components/common/MultiSourceImage.vue";
import ToggableFavoriteIcon from "@/components/common/ToggableFavoriteIcon.vue";
import LoadArtistListItem from "@/components/common/LoadArtistListItem.vue";
import ArtistDiscography from "@/components/common/ArtistDiscography.vue";
import RadioIcon from "@/components/icons/RadioIcon.vue";
import { usePlaybackStore } from "@/store/playback";
import RadioBuilderModal from "@/components/common/RadioBuilderModal.vue";

const props = defineProps({
  artistId: {
    type: String,
    required: true,
  },
});

const artist = ref(null);
const coverUrls = ref(null);
const isArtistLiked = ref(false);
const showRadioBuilder = ref(false);
const userStore = useUserStore();
const staticsStore = useStaticsStore();
const remoteStore = useRemoteStore();
const playback = usePlaybackStore();
const bioTextRef = ref(null);
const bioExpanded = ref(false);
const bioOverflows = ref(false);

const enrichmentLabel = computed(() => {
  const status = artist.value?.enrichment_status?.status;
  if (status === "queued") return "Enrichment queued";
  if (status === "running") return "Enrichment running";
  if (status === "failed") return "Enrichment failed";
  return null;
});

const artistEnrichment = computed(() => artist.value?.enrichment || null);

const shortBio = computed(() => {
  const profile = artistEnrichment.value?.profile;
  return profile?.summary || profile?.bio || null;
});

const updateBioOverflow = async () => {
  await nextTick();

  const element = bioTextRef.value;
  if (!element) {
    bioOverflows.value = false;
    return;
  }

  const styles = window.getComputedStyle(element);
  const lineHeight = Number.parseFloat(styles.lineHeight);
  const maxCollapsedHeight = Number.isFinite(lineHeight) ? lineHeight * 3 : 0;
  bioOverflows.value =
    maxCollapsedHeight > 0 && element.scrollHeight > maxCollapsedHeight + 1;
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

const formatPlaceAndDate = (place, date) => {
  return [place, date].filter(Boolean).join(", ");
};

const lifeSummary = computed(() => {
  const profile = artistEnrichment.value?.profile;
  if (!profile) return null;

  const birthPlace =
    profile.birth_place ||
    profile.birthplace ||
    (profile.is_person !== false
      ? profile.origin_place || profile.origin_country
      : null);
  const deathPlace =
    profile.death_place || profile.deathplace || profile.place_of_death;

  const birth = formatPlaceAndDate(
    birthPlace,
    formatEnrichmentDate(profile.birth_date),
  );
  const death = formatPlaceAndDate(
    deathPlace,
    formatEnrichmentDate(profile.death_date),
  );

  return [birth, death].filter(Boolean).join(" - ") || null;
});

watch(
  shortBio,
  () => {
    bioExpanded.value = false;
    updateBioOverflow();
  },
  { immediate: true },
);

let artistDataUnwatcher = null;

const fetchData = async (id) => {
  if (artistDataUnwatcher) {
    artistDataUnwatcher();
    artistDataUnwatcher = null;
  }
  if (!id) return;

  artistDataUnwatcher = watch(
    staticsStore.getArtist(id),
    (newData) => {
      if (newData && newData.item && typeof newData.item === "object") {
        coverUrls.value = chooseArtistCoverImageUrl(newData.item);
        artist.value = newData.item;
      }
    },
    { immediate: true },
  );
};

watch(
  [() => userStore.likedArtistsIds, artist],
  ([likedArtis, artistData]) => {
    if (likedArtis && artistData) {
      isArtistLiked.value = likedArtis.includes(props.artistId);
    }
  },
  { immediate: true },
);

const handleClickOnFavoriteIcon = () => {
  userStore.setArtistIsLiked(props.artistId, !isArtistLiked.value);
};

const handleClickOnArtistRadio = () => {
  playback.setRadioFromItem("artist", props.artistId);
};

watch(
  () => props.artistId,
  (newId) => {
    fetchData(newId);
    if (newId) {
      remoteStore.recordImpression("artist", newId);
    }
  },
);

onMounted(() => {
  fetchData(props.artistId);
  remoteStore.recordImpression("artist", props.artistId);
  updateBioOverflow();
  window.addEventListener("resize", updateBioOverflow);
});

onUnmounted(() => {
  window.removeEventListener("resize", updateBioOverflow);
});
</script>

<style scoped>
.topSection {
  display: grid;
  grid-template-columns: minmax(180px, 300px) minmax(0, 1fr);
  gap: clamp(20px, 3vw, 36px);
  align-items: start;
  padding: clamp(18px, 3vw, 32px);
  border: 1px solid var(--surface-border);
  border-radius: 8px;
  background: linear-gradient(
      135deg,
      rgba(58, 134, 255, 0.16),
      rgba(17, 20, 22, 0.58) 45%
    ),
    var(--surface-raised);
}

.coverImage {
  width: 100%;
  aspect-ratio: 1;
  height: auto;
  object-fit: cover;
  border-radius: 50%;
  box-shadow: var(--shadow-lg);
  overflow: hidden;
}

.artistInfoColum {
  min-width: 0;
  align-self: stretch;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  justify-content: space-between;
  gap: 24px;
  margin: 0;
}

.artistIdentity {
  min-width: 0;
}

.artistLifeSummary {
  margin: 12px 0 0;
  color: var(--text-muted);
  font-size: clamp(0.95rem, 1.3vw, 1.1rem);
  font-weight: 650;
  line-height: 1.35;
  overflow-wrap: anywhere;
}

.artistBioBlock {
  max-width: 860px;
  margin-top: 18px;
}

.artistBioText {
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 3;
  margin: 0;
  overflow: hidden;
  color: var(--text-muted);
  font-size: 0.96rem;
  line-height: 1.5;
}

.artistBioText.expanded {
  display: block;
  -webkit-line-clamp: unset;
  overflow: visible;
}

.artistBioToggle {
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

.artistBioToggle:hover {
  color: var(--spotify-green);
}

.artistName {
  margin: 0;
  color: var(--text-base);
  font-size: clamp(2rem, 4.6vw, 4.6rem);
  font-weight: 900;
  line-height: 0.96;
  letter-spacing: 0;
}

.relatedArtistsContainer {
  width: 100%;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: 8px;
  overflow: visible;
  margin: 16px 0;
}

.discographyContainer {
  margin: 18px 0 0;
}

.verticalFiller {
  display: none;
}

.artistActions {
  display: flex;
  margin-top: auto;
  align-items: center;
  flex-wrap: wrap;
  gap: 12px;
}

.artistActions :deep(.bigIcon) {
  width: 42px;
  height: 42px;
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

.advancedRadioButton {
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

.advancedRadioButton:hover {
  background: var(--surface-hover);
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

<style scoped>
.enrichmentStatus {
  margin: 12px 0;
  color: var(--text-muted);
  font-size: 0.9rem;
}
</style>
