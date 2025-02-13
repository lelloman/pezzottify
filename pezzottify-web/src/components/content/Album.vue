<template>
  <div v-if="data">
    <h2>Data for ID: {{ albumId }}</h2>
    <pre>{{ data }}</pre>
  </div>
  <div v-else>
    <p>Loading {{ albumId }}...</p>
  </div>
</template>

<script setup>
import { ref, watch, onMounted } from 'vue';
import axios from 'axios';

const props = defineProps({
  albumId: {
    type: String,
    required: true,
  }
});

const data = ref(null);

const fetchData = async (id) => {
  if (!id) return;
  data.value = null;
  try {
    const response = await axios.get(`/v1/content/album/${id}`);
    data.value = response.data;
  } catch (error) {
    console.error('Error fetching data:', error);
  }
};

watch(() => props.albumId, (newId) => {
  fetchData(newId);
});

onMounted(() => {
  fetchData(props.albumId);
});
</script>
