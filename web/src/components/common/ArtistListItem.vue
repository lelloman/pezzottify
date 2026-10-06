<template>
  <!-- display: contents keeps the row's layout; the menu sits outside the clickable row. -->
  <div class="artistListItem">
    <div
      class="searchResultRow"
      :data-id="artist.id"
      @click="handleClick(artist)"
      @contextmenu.prevent="
        contextMenu?.openMenu($event, 'artist', artist.id, artist.name)
      "
    >
      <MultiSourceImage
        :urls="chooseSmallArtistImageUrl(artist)"
        class="searchResultRoundImage"
      />
      <div class="artistIdentity">
        <h3 class="title">{{ artist.name }}</h3>
        <p v-if="library" class="librarySubtitle">Artist</p>
      </div>
    </div>
    <EntityContextMenu ref="contextMenu" />
  </div>
</template>

<script setup>
import "@/assets/search.css";
import { useRouter } from "vue-router";
import { chooseSmallArtistImageUrl } from "@/utils";
import { ref } from "vue";
import MultiSourceImage from "./MultiSourceImage.vue";
import EntityContextMenu from "@/components/common/contextmenu/EntityContextMenu.vue";

defineProps({
  library: Boolean,
  artist: {
    type: Object,
    required: true,
  },
});

const router = useRouter();
const contextMenu = ref(null);

const handleClick = (artist) => {
  router.push("/artist/" + artist.id);
};
</script>

<style scoped>
.artistIdentity {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.artistListItem {
  display: contents;
}

.relatedArtistWrapper {
  min-width: 0;
  margin: 0;
}

.searchResultRoundImage {
  width: 52px;
  height: 52px;
  border-radius: 50%;
  margin-right: 12px;
}

.title {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  margin: 0;
  font-size: var(--text-lg);
  font-weight: 700;
  color: #ffffff !important;
}
</style>
