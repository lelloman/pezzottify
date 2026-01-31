<template>
  <div>
    <ClickableArtistsNames
      v-if="loadedArtistsIdsNames.length > 0"
      :prefix="prefix"
      :artistsIdsNames="loadedArtistsIdsNames"
    />
    <div v-if="isLoading">
      <span v-if="loadedArtistsIdsNames.length > 0"
        >Loading more artists...</span
      >
      <span v-else>Loading artists...</span>
    </div>
  </div>
</template>

<script setup>
import { ref, watch, reactive } from "vue";
import ClickableArtistsNames from "./ClickableArtistsNames.vue";
import { useStaticsStore } from "@/store/statics";

const props = defineProps({
  prefix: {
    type: String,
  },
  artistsIds: {
    type: Array,
    required: true,
  },
});

const staticsStore = useStaticsStore();

const isLoading = ref(true);
const loadedArtistsIdsNames = ref([]);
const artistsRefs = reactive([]);

// Watch the reactive array for changes to detect when artists load
watch(
  artistsRefs,
  (refs) => {
    if (refs.length === 0) return;

    // Update loaded artists immediately when any artist loads
    loadedArtistsIdsNames.value = refs
      .filter((ref) => ref.item && typeof ref.item === "object")
      .map((ref) => [ref.item.id, ref.item.name]);

    // Update loading state when all artists are loaded
    isLoading.value = !refs.every(
      (ref) => ref.item && typeof ref.item === "object",
    );
  },
  { deep: true, immediate: true },
);

// Watch artistsIds prop for changes (handles both initial load and track changes)
watch(
  () => props.artistsIds,
  (ids) => {
    isLoading.value = true;
    loadedArtistsIdsNames.value = [];
    artistsRefs.length = 0;

    if (!ids || !Array.isArray(ids) || ids.length === 0) {
      isLoading.value = false;
      return;
    }

    ids.forEach((artistId) => {
      artistsRefs.push(staticsStore.getArtist(artistId));
    });
  },
  { immediate: true, deep: true },
);
</script>
