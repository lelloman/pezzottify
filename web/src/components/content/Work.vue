<template>
  <div class="workPage">
    <p v-if="loading && !work">Loading work…</p>
    <p v-if="error" role="alert">
      {{ error }} <button @click="load">Retry</button>
    </p>
    <template v-if="work">
      <p class="eyebrow">Work · {{ work.kind }}</p>
      <h1>{{ work.title }}</h1>
      <p>{{ work.creators.join(", ") }}</p>
      <p v-if="work.catalog_number">{{ work.catalog_number }}</p>
      <h2>Performances and versions</h2>
      <p v-if="!tracks.length && !loading">No tracks available.</p>
      <ul>
        <li v-for="entry in tracks" :key="entry.track.id">
          <RouterLink
            :to="{ name: 'track', params: { trackId: entry.track.id } }"
            >{{ entry.track.name }}</RouterLink
          >
          <span>
            — {{ entry.artists.map((a) => a.artist.name).join(", ") }} ·
          </span>
          <RouterLink
            :to="{ name: 'album', params: { albumId: entry.album.id } }"
            >{{ entry.album.name }}</RouterLink
          >
        </li>
      </ul>
      <button v-if="hasMore" :disabled="loading" @click="load">
        {{ loading ? "Loading…" : "Load more" }}
      </button>
    </template>
  </div>
</template>

<script setup>
import { ref, onMounted } from "vue";
import axios from "axios";
const props = defineProps({ workId: { type: String, required: true } });
const work = ref(null);
const tracks = ref([]);
const loading = ref(false);
const error = ref("");
const hasMore = ref(false);
let offset = 0;
async function load() {
  if (loading.value) return;
  loading.value = true;
  error.value = "";
  try {
    const { data } = await axios.get(
      `/v1/content/work/${encodeURIComponent(props.workId)}`,
      { params: { limit: 50, offset } },
    );
    work.value = data.work;
    tracks.value.push(...data.tracks);
    hasMore.value = data.has_more;
    offset = data.next_offset;
  } catch (e) {
    error.value =
      e.response?.status === 404
        ? "Work not found."
        : "Could not load this work.";
  } finally {
    loading.value = false;
  }
}
onMounted(load);
</script>

<style scoped>
.workPage {
  padding: 32px;
  color: var(--text-primary);
}
.eyebrow {
  color: var(--text-secondary);
  text-transform: uppercase;
}
li {
  margin: 16px 0;
}
a {
  color: inherit;
}
</style>
