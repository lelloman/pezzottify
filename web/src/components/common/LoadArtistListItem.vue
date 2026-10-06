<template>
  <div class="relatedArtistWrapper">
    <div v-if="loading" class="artistState">Loading</div>
    <SearchEntityCard
      v-else-if="artistData && card"
      hideSubtitle
      :result="{ ...artistData, type: 'Artist' }"
      @contextmenu.prevent="
        entityMenu?.openMenu($event, 'artist', artistData.id, artistData.name)
      "
    />
    <ArtistListItem
      v-else-if="artistData"
      :data-id="artistData.id"
      :artist="artistData"
      :library="library"
    />
    <div v-else-if="error" class="artistState errorState">
      Error. {{ error }}
    </div>
    <Teleport v-if="card" to="body"
      ><EntityContextMenu ref="entityMenu"
    /></Teleport>
  </div>
</template>

<script setup>
import { ref, watch } from "vue";
import SearchEntityCard from "@/components/search/SearchEntityCard.vue";
import EntityContextMenu from "./contextmenu/EntityContextMenu.vue";
import ArtistListItem from "@/components/common/ArtistListItem.vue";
import { useStaticsStore } from "@/store/statics";

const staticsStore = useStaticsStore();

const props = defineProps({
  library: Boolean,
  card: Boolean,
  artistId: {
    type: String,
    required: true,
  },
});

const entityMenu = ref(null);
const artistData = ref(null);
const loading = ref(true);
const error = ref(null);

watch(
  staticsStore.getArtist(props.artistId),
  (newData) => {
    loading.value = newData && newData.loading;
    if (newData && newData.item && typeof newData.item === "object") {
      artistData.value = newData.item;
    }
  },
  { immediate: true },
);
</script>

<style scoped>
.relatedArtistWrapper {
  min-width: 0;
  margin: 0;
  color: #ffffff !important;
}

.artistState {
  display: flex;
  align-items: center;
  min-height: 64px;
  padding: 10px 12px;
  border-radius: 8px;
  color: var(--text-subdued);
  font-size: 0.82rem;
  font-weight: 700;
}

.errorState {
  color: #ffb4a8;
}
</style>
