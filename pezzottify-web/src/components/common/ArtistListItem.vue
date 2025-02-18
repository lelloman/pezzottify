<template>
  <div class="searchResultRow" :data-id="artist.id" @click="handleClick(artist)">
    <MultiSourceImage :urls="chooseSmallArtistImageUrl(artist)" class="searchResultRoundImage" />
    <h3 class="title">{{ artist.name }}</h3>
  </div>
</template>

<script setup>
import { useRouter } from 'vue-router';
import { chooseSmallArtistImageUrl } from '@/utils';
import MultiSourceImage from './MultiSourceImage.vue';

defineProps({
  artist: {
    type: Object,
    required: true,
  }
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
  height: 100%;
  align-content: center;
}

.searchResultRoundImage {
  width: 80px;
  height: 80px;
  border-radius: 40px;
  margin-right: 10px;
}

.title {
  margin: 0;
  font-size: 16px;
  font-weight: bold;
}
</style>
