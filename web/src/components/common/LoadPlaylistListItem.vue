<template>
  <div class="playlistWrapper">
    <div v-if="loading">Loading...</div>
    <div v-else-if="localPlaylistdata" class="playlistItem searchResultRow" @click.stop="handleClick">
      <h2>{{ localPlaylistdata.name }} ({{ localPlaylistdata.tracks.length }})</h2>
    </div>
    <div v-else-if="error">Error. {{ error }}</div>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue';
import { useRouter } from 'vue-router';
import { useUserStore } from '@/store/user';

const router = useRouter();
const userStore = useUserStore();

const props = defineProps({
  playlistData: {
    required: true,
  }
});

const localPlaylistdata = ref(null);
const loading = ref(true);
const error = ref(null);

const fetchPlyalistData = async (id) => {
  loading.value = true;
  userStore.loadPlaylistData(id, (data) => {
    console.log("LoadPLaylistListItem UserPlaylist data loaded:");
    console.log(data);
    if (data) {
      localPlaylistdata.value = data;
    } else {
      error.value = "Error loading playlist data";
    }
    loading.value = false;
  });
};

const handleClick = () => {
  router.push(`/playlist/${localPlaylistdata.value.id}`);
};

onMounted(() => {
  const isString = typeof (props.playlistData) == 'string';
  console.log("LoadPLaylistListItem mounted, data (isString ", isString, ") :");
  console.log(props.playlistData);
  if (isString) {
    fetchPlyalistData(props.playlistData);
  } else {
    localPlaylistdata.value = props.playlistData;
    loading.value = false;
  }
});

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
