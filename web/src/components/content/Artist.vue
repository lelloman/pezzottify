<template>
  <div v-if="artist">
    <div class="topSection">
      <MultiSourceImage class="coverImage" :urls="coverUrls" />
      <div class="artistInfoColum">
        <h1 class="artistName">{{ artist.name }}</h1>
        <div class="verticalFiller"></div>
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
    <section v-if="hasEnrichmentData" class="enrichmentPanel">
      <div v-if="shortBio" class="enrichmentText">
        <h2>About</h2>
        <p>{{ shortBio }}</p>
      </div>
      <div v-if="enrichmentFacts.length" class="enrichmentFacts">
        <div
          v-for="fact in enrichmentFacts"
          :key="fact.label"
          class="enrichmentFact"
        >
          <span>{{ fact.label }}</span>
          <strong>{{ fact.value }}</strong>
        </div>
      </div>
      <div v-if="tagLabels.length" class="enrichmentGroup">
        <h3>Styles</h3>
        <div class="enrichmentChips">
          <span v-for="tag in tagLabels" :key="tag" class="enrichmentChip">{{
            tag
          }}</span>
        </div>
      </div>
      <div v-if="contributorLabels.length" class="enrichmentGroup">
        <h3>Credits</h3>
        <div class="enrichmentList">
          <span v-for="credit in contributorLabels" :key="credit">{{
            credit
          }}</span>
        </div>
      </div>
    </section>
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
import { computed, ref, watch, onMounted } from "vue";
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

const enrichmentLabel = computed(() => {
  const status = artist.value?.enrichment_status?.status;
  if (status === "queued") return "Enrichment queued";
  if (status === "running") return "Enrichment running";
  if (status === "failed") return "Enrichment failed";
  return null;
});

const artistEnrichment = computed(() => artist.value?.enrichment || null);

const titleCase = (value) => {
  if (!value) return null;
  return String(value)
    .replace(/_/g, " ")
    .replace(/\b\w/g, (char) => char.toUpperCase());
};

const shortBio = computed(() => {
  const profile = artistEnrichment.value?.profile;
  return profile?.summary || profile?.bio || null;
});

const enrichmentFacts = computed(() => {
  const profile = artistEnrichment.value?.profile;
  if (!profile) return [];

  const placeLabel = profile.is_person ? "Birthplace" : "Origin";
  const place = profile.origin_place || profile.origin_country;

  return [
    { label: "Born", value: profile.birth_date },
    { label: "Died", value: profile.death_date },
    { label: placeLabel, value: place },
    { label: "Founded", value: profile.foundation_date },
    { label: "Dissolved", value: profile.dissolution_date },
    { label: "Language", value: titleCase(profile.primary_language) },
  ].filter((fact) => fact.value);
});

const tagLabels = computed(() => {
  const seen = new Set();
  return (artistEnrichment.value?.tags || [])
    .map((tag) => tag.tag)
    .filter((tag) => {
      if (!tag || seen.has(tag.toLowerCase())) return false;
      seen.add(tag.toLowerCase());
      return true;
    });
});

const contributorLabels = computed(() => {
  return (artistEnrichment.value?.contributors || []).map((contributor) => {
    const role = titleCase(contributor.role);
    return role
      ? `${contributor.contributor_name} - ${role}`
      : contributor.contributor_name;
  });
});

const hasEnrichmentData = computed(() => {
  return Boolean(
    shortBio.value ||
      enrichmentFacts.value.length ||
      tagLabels.value.length ||
      contributorLabels.value.length,
  );
});

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
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 12px;
  margin: 0;
}

.artistName {
  margin: 0;
  color: var(--text-base);
  font-size: clamp(2rem, 4.6vw, 4.6rem);
  font-weight: 900;
  line-height: 0.96;
  letter-spacing: 0;
}

.enrichmentPanel {
  margin: 18px 0 8px;
  padding: 18px 0;
  border-top: 1px solid var(--surface-border);
  border-bottom: 1px solid var(--surface-border);
  color: var(--text-base);
}

.enrichmentText h2,
.enrichmentGroup h3 {
  margin: 0 0 8px;
  color: var(--text-base);
  font-size: 1rem;
  font-weight: 800;
  letter-spacing: 0;
}

.enrichmentText p {
  max-width: 840px;
  margin: 0 0 16px;
  color: var(--text-muted);
  font-size: 0.96rem;
  line-height: 1.55;
}

.enrichmentFacts {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
  gap: 10px 18px;
  margin: 0 0 16px;
}

.enrichmentFact {
  min-width: 0;
}

.enrichmentFact span {
  display: block;
  color: var(--text-muted);
  font-size: 0.78rem;
  font-weight: 700;
  text-transform: uppercase;
}

.enrichmentFact strong {
  display: block;
  margin-top: 3px;
  color: var(--text-base);
  font-size: 0.92rem;
  line-height: 1.3;
  overflow-wrap: anywhere;
}

.enrichmentGroup {
  margin-top: 14px;
}

.enrichmentChips,
.enrichmentList {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.enrichmentChip,
.enrichmentList span,
.enrichmentList a {
  min-height: 28px;
  padding: 5px 10px;
  border: 1px solid var(--surface-border);
  border-radius: 999px;
  color: var(--text-muted);
  background: rgba(255, 255, 255, 0.04);
  font-size: 0.84rem;
  line-height: 1.2;
  text-decoration: none;
}

.enrichmentList a:hover {
  color: var(--text-base);
  border-color: var(--surface-border-strong);
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
