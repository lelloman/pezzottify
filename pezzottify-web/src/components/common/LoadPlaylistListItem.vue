<template>
  <div class="playlistWrapper">
    <div v-if="loading">Loading...</div>
    <div v-else-if="playlistData" class="playlistItem searchResultRow" @click.stop="handleClick">
      <h2>{{ playlistData.name }} ({{ playlistData.tracks.length }})</h2>
    </div>
    <div v-else-if="error">Error. {{ error }}</div>
  </div>
</template>

<script setup>
import { ref, onMounted } from 'vue';
import axios from 'axios';
import { useRouter } from 'vue-router';

const router = useRouter();

const props = defineProps({
  playlistId: {
    type: String,
    required: true,
  }
});

const playlistData = ref(null);
const loading = ref(true);
const error = ref(null);

const fetchPlyalistData = async (id) => {
  try {
    const response = await axios.get(`/v1/user/playlist/${id}`);
    playlistData.value = response.data;
  } catch (err) {
    error.value = err.message;
  } finally {
    loading.value = false;
  }
};

const handleClick = () => {
  router.push(`/playlist/${props.playlistId}`);
};

onMounted(() => {
  fetchPlyalistData(props.playlistId);
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
