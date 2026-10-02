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
      <h3 class="title">{{ artist.name }}</h3>
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
  font-size: 0.9rem;
  font-weight: 850;
  color: #ffffff !important;
}
</style>
