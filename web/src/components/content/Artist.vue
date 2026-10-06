<template>
  <DetailPage
    v-if="artist"
    :title="artist.name"
    kind="Artist"
    @artwork-contextmenu.prevent="
      entityMenu?.openMenu($event, 'artist', artistId, artist.name)
    "
    :imageUrls="coverUrls || []"
    banner
  >
    <template #meta
      ><span v-if="lifeSummary">{{ lifeSummary }}</span
      ><span v-if="artist.genres?.length">{{
        artist.genres.join(" · ")
      }}</span></template
    >
    <template #actions>
      <DetailActions
        playLabel="Play greatest hits"
        showSave
        :saved="isArtistLiked"
        @play="playback.setArtistGreatestHits(artistId)"
        @save="handleClickOnFavoriteIcon"
        :disabled="playback.radioCreationState.status === 'creating'"
      >
        <template #more>
          <button type="button" @click="handleClickOnArtistRadio">
            <RadioIcon /> Start radio
          </button>
          <button
            class="advancedRadioButton"
            @click.stop="showRadioBuilder = true"
          >
            Customize radio
          </button>
          <button
            class="advancedRadioButton steerButton"
            title="Steer the current queue toward this artist"
            @click.stop="showDestinationPrompt = true"
          >
            <SteeringWheelIcon class="steerButtonIcon" />
            Steer here
          </button>
          <button
            v-if="canAddToDestinationMix"
            class="advancedRadioButton steerButton"
            title="Add this artist to the current destination mix"
            @click.stop="
              playback.addGravityDestinationComponent({
                entity_type: 'artist',
                entity_id: artistId,
                label: artist.name,
              })
            "
          >
            <SteeringWheelIcon class="steerButtonIcon" />
            Add to mix
          </button>
        </template>
      </DetailActions>
    </template>

    <div class="discographyContainer">
      <ArtistDiscography :artistId="artistId" />
    </div>
    <div class="discographyContainer">
      <ArtistDiscography :artistId="artistId" :appearsOn="true" />
    </div>
    <section
      v-if="
        shortBio ||
        ['queued', 'running', 'failed', 'failed_enrichment'].includes(
          artist.enrichment_status?.status,
        )
      "
      class="detailSupporting"
    >
      <h2 class="detailSectionTitle">About this artist</h2>
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
      <EnrichmentStatusIndicator
        :status="artist.enrichment_status"
        entityType="artist"
      />
    </section>
    <section v-if="artist.related?.length" class="detailSupporting">
      <h2 class="detailSectionTitle">Related artists</h2>
      <div class="relatedArtistsContainer">
        <LoadArtistListItem
          v-for="artistId in artist.related"
          :key="artistId"
          :artistId="artistId"
        />
      </div>
    </section>
    <DestinationStepsPrompt
      :isOpen="showDestinationPrompt"
      :reference="{
        entity_type: 'artist',
        entity_id: artistId,
        label: artist.name,
      }"
      @close="showDestinationPrompt = false"
    />
    <EntityContextMenu ref="entityMenu" />
    <RadioBuilderModal
      :isOpen="showRadioBuilder"
      seedEntityType="artist"
      :seedEntityId="artistId"
      @close="showRadioBuilder = false"
    />
  </DetailPage>

  <div v-else>
    <p>Loading {{ artistId }}...</p>
  </div>
</template>

<script setup>
import DetailPage from "@/components/common/DetailPage.vue";
import DetailActions from "@/components/common/DetailActions.vue";
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { MAX_COMPONENTS } from "@/utils/gravity";
import { chooseArtistCoverImageUrl } from "@/utils";
import { useUserStore } from "@/store/user.js";
import { useStaticsStore } from "@/store/statics.js";
import { useRemoteStore } from "@/store/remote.js";
import LoadArtistListItem from "@/components/common/LoadArtistListItem.vue";
import ArtistDiscography from "@/components/common/ArtistDiscography.vue";
import RadioIcon from "@/components/icons/RadioIcon.vue";
import { usePlaybackStore } from "@/store/playback";
import RadioBuilderModal from "@/components/common/RadioBuilderModal.vue";
import DestinationStepsPrompt from "@/components/common/DestinationStepsPrompt.vue";
import EntityContextMenu from "@/components/common/contextmenu/EntityContextMenu.vue";
import SteeringWheelIcon from "@/components/icons/SteeringWheelIcon.vue";
import EnrichmentStatusIndicator from "@/components/common/EnrichmentStatusIndicator.vue";

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
const showDestinationPrompt = ref(false);
const entityMenu = ref(null);
const userStore = useUserStore();
const staticsStore = useStaticsStore();
const remoteStore = useRemoteStore();
const playback = usePlaybackStore();
const canAddToDestinationMix = computed(() => {
  const destination = playback.currentGravity?.destination;
  return Boolean(destination?.length && destination.length < MAX_COMPONENTS);
});
const bioTextRef = ref(null);
const bioExpanded = ref(false);
const bioOverflows = ref(false);

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
  font-weight: 700;
  cursor: pointer;
}

.artistBioToggle:hover {
  color: var(--spotify-green);
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

.advancedRadioButton {
  width: fit-content;
  min-height: 38px;
  padding: 0 16px;
  border: 1px solid var(--surface-border-strong);
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.04);
  color: var(--text-base);
  font-size: 0.86rem;
  font-weight: 700;
  cursor: pointer;
}

.advancedRadioButton:hover {
  background: var(--surface-hover);
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
