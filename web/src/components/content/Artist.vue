<template>
  <div v-if="artist">
    <div class="topSection">
      <MultiSourceImage class="coverImage" :urls="coverUrls" />
      <div class="artistInfoColum">
        <h1 class="artistName">{{ artist.name }}</h1>
        <div class="verticalFiller"></div>
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
import { ref, watch, onMounted } from "vue";
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
  align-items: end;
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
