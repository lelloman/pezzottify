<template>
  <div class="relatedArtistWrapper">
    <div v-if="loading">Loading...</div>
    <ArtistListItem v-else-if="artistData" :data-id="artistData.id" :artist="artistData" />
    <div v-else-if="error">Error. {{ error }}</div>
  </div>
</template>

<script setup>
import { ref, watch } from 'vue';
import ArtistListItem from '@/components/common/ArtistListItem.vue';
import { useStaticsStore } from '@/store/statics';

const staticsStore = useStaticsStore();

const props = defineProps({
  artistId: {
    type: String,
    required: true,
  }
});

const artistData = ref(null);
const loading = ref(true);
const error = ref(null);

watch(staticsStore.getArtist(props.artistId), (newData) => {
  loading.value = newData && newData.loading;
  if (newData && newData.item && typeof newData.item === 'object') {
    artistData.value = newData.item;
  }
}, { immediate: true });

</script>

<style scoped>
.relatedArtistWrapper {
  min-width: 200px;
  margin: 10px;
  height: 100%;
  align-content: center;
}
</style>
