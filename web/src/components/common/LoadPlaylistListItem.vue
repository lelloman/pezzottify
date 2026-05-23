<template>
  <div class="playlistWrapper">
    <div v-if="loading">Loading...</div>
    <div
      v-else-if="playlist"
      class="playlistItem searchResultRow"
      @click.stop="handleClick"
    >
      <h2>{{ playlist.name }} ({{ playlist.tracks?.length || 0 }})</h2>
    </div>
    <div v-else-if="error">Error. {{ error }}</div>
  </div>
</template>

<script setup>
import "@/assets/search.css";
import { ref, onMounted, onBeforeUnmount, computed } from "vue";
import { useRouter } from "vue-router";
import { useUserStore } from "@/store/user";

const router = useRouter();
const userStore = useUserStore();

const props = defineProps({
  playlistId: {
    type: String,
    required: true,
  },
});

const loading = ref(true);
const error = ref(null);
const playlistRef = ref(null);

onMounted(() => {
  // Get the reference on mount
  playlistRef.value = userStore.getPlaylistRef(props.playlistId);

  userStore
    .loadPlaylistData(props.playlistId)
    .finally(() => (loading.value = false));
});

onBeforeUnmount(() => {
  // Release the reference when component is unmounted
  if (playlistRef.value) {
    userStore.putPlaylistRef(props.playlistId);
  }
});

const playlist = computed(() => {
  return playlistRef.value?.value;
});

const handleClick = () => {
  if (playlist.value) {
    router.push(`/playlist/${playlist.value.id}`);
  }
};
</script>

<style scoped>
.playlistWrapper {
  min-width: 0;
  margin: 0;
  color: #ffffff !important;
}

.playlistItem {
  padding: 12px;
  min-height: 56px;
  border-radius: 8px;
  color: var(--text-base) !important;
}

.playlistItem h2 {
  color: var(--text-base) !important;
  font-size: 0.9rem;
  font-weight: 850;
  margin: 0;
}
</style>
