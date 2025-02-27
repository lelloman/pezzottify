<template>
  <div class="playlistWrapper">
    <div v-if="loading">Loading...</div>
    <div v-else-if="playlist" class="playlistItem searchResultRow" @click.stop="handleClick">
      <h2>{{ playlist.name }} ({{ playlist.tracks?.length || 0 }})</h2>
    </div>
    <div v-else-if="error">Error. {{ error }}</div>
  </div>
</template>

<script setup>
import { ref, onMounted, computed } from 'vue';
import { useRouter } from 'vue-router';
import { useUserStore } from '@/store/user';

const router = useRouter();
const userStore = useUserStore();

const props = defineProps({
  playlistId: {
    type: String,
    required: true,
  }
});

const loading = ref(true);
const error = ref(null);

onMounted(() => {
  userStore.loadPlaylistData(props.playlistId)
    .finally(() => loading.value = false);
});

const playlist = computed(() => {
  const playlistRef = userStore.getPlaylistRef(props.playlistId);
  return playlistRef?.value;
});

const handleClick = () => {
  if (playlist.value) {
    router.push(`/playlist/${playlist.value.id}`);
  }
};
</script>

<style scoped>
.playlistWrapper {
  min-width: 200px;
  margin: 10px;
  height: 100%;
  align-content: center;
}

.playlistItem {
  padding: 16px;
}
</style>
