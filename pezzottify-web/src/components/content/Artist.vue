<template>
  <div v-if="data">
    <h2>Data for ID: {{ artistId }}</h2>
    <pre>{{ data }}</pre>
  </div>
  <div v-else>
    <p>Loading {{ artistId }}...</p>
  </div>
</template>

<script setup>
import { ref, watch, onMounted } from 'vue';
import axios from 'axios';

const props = defineProps({
  artistId: {
    type: String,
    required: true,
  }
});

const data = ref(null);

const fetchData = async (id) => {
  if (!id) return;
  data.value = null;
  try {
    const response = await axios.get(`/v1/content/artist/${id}`);
    data.value = response.data;
  } catch (error) {
    console.error('Error fetching data:', error);
  }
};

watch(() => props.artistId, (newId) => {
  fetchData(newId);
});

onMounted(() => {
  fetchData(props.artistId);
});
</script>
