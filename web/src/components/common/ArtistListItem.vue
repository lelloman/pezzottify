<template>
  <div
    class="searchResultRow"
    :data-id="artist.id"
    @click="handleClick(artist)"
  >
    <MultiSourceImage
      :urls="chooseSmallArtistImageUrl(artist)"
      class="searchResultRoundImage"
    />
    <h3 class="title">{{ artist.name }}</h3>
  </div>
</template>

<script setup>
import "@/assets/search.css";
import { useRouter } from "vue-router";
import { chooseSmallArtistImageUrl } from "@/utils";
import MultiSourceImage from "./MultiSourceImage.vue";

defineProps({
  artist: {
    type: Object,
    required: true,
  },
});

const router = useRouter();

const handleClick = (artist) => {
  router.push("/artist/" + artist.id);
};
</script>

<style scoped>
.relatedArtistWrapper {
  min-width: 200px;
  margin: 10px;
}

.searchResultRoundImage {
  width: 52px;
  height: 52px;
  border-radius: 50%;
  margin-right: 12px;
}

.title {
  margin: 0;
  font-size: 0.9rem;
  font-weight: 850;
  color: #ffffff !important;
}
</style>
