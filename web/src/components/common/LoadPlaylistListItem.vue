<template>
  <div class="playlistWrapper">
    <div v-if="loading" class="playlistState">Loading</div>
    <div
      v-else-if="playlist"
      class="playlistItem searchResultRow"
      @click.stop="handleClick"
    >
      <div class="playlistIcon">P</div>
      <div class="playlistMeta">
        <h2>{{ playlist.name }}</h2>
        <span>{{ playlist.tracks?.length || 0 }} tracks</span>
      </div>
    </div>
    <div v-else-if="error" class="playlistState errorState">
      Error. {{ error }}
    </div>
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
  display: grid;
  grid-template-columns: 44px minmax(0, 1fr);
  gap: 12px;
  min-height: 62px;
  padding: 8px;
  border-radius: 8px;
  color: var(--text-base) !important;
}

.playlistIcon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 44px;
  height: 44px;
  border-radius: 7px;
  background: rgba(29, 185, 84, 0.16);
  color: var(--spotify-green);
  font-size: 1rem;
  font-weight: 900;
}

.playlistMeta {
  display: flex;
  min-width: 0;
  flex-direction: column;
  justify-content: center;
  gap: 3px;
}

.playlistItem h2 {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text-base) !important;
  font-size: 0.9rem;
  font-weight: 850;
  margin: 0;
}

.playlistItem span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text-subdued);
  font-size: 0.76rem;
  font-weight: 620;
}

.playlistState {
  display: flex;
  align-items: center;
  min-height: 62px;
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
