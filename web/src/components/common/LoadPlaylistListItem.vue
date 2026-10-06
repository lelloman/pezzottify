<template>
  <div class="playlistWrapper">
    <div v-if="loading" class="playlistState">Loading</div>
    <div
      v-else-if="playlist"
      class="playlistItem searchResultRow"
      @click.stop="handleClick"
    >
      <div class="playlistIcon">
        <svg
          v-if="library"
          viewBox="0 0 24 24"
          width="24"
          height="24"
          fill="currentColor"
          aria-hidden="true"
        >
          <path
            d="M9 4v11.2a3.5 3.5 0 1 0 2 3.2V8l8-2v7.2a3.5 3.5 0 1 0 2 3.2V1z"
          /></svg
        ><template v-else>P</template>
      </div>
      <div class="playlistMeta">
        <h2>{{ playlist.name }}</h2>
        <span
          >{{ library ? "Playlist · " : ""
          }}{{ playlist.tracks?.length || 0 }} tracks</span
        >
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
  library: Boolean,
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
.playlistIcon:has(svg) {
  background: #282828;
  color: #b3b3b3;
  border-radius: 4px;
}

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
  font-weight: 700;
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
  font-size: var(--text-lg);
  font-weight: 700;
  margin: 0;
}

.playlistItem span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--text-subdued);
  font-size: var(--text-sm);
  font-weight: 400;
}

.playlistState {
  display: flex;
  align-items: center;
  min-height: 62px;
  padding: 10px 12px;
  border-radius: 8px;
  color: var(--text-subdued);
  font-size: var(--text-sm);
  font-weight: 700;
}

.errorState {
  color: #ffb4a8;
}
</style>
