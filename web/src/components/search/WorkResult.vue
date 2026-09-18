<template>
  <RouterLink
    class="searchResultRow workResult"
    :to="{ name: 'work', params: { workId: result.id } }"
  >
    <WorkArtwork
      class="searchResultImage"
      :artistIds="result.creator_artist_ids || []"
    />
    <div class="column">
      <span class="title" :title="result.title">{{ result.title }}</span>
      <span class="creators" :title="creators">{{
        creators || "Unknown creator"
      }}</span>
    </div>
    <span
      v-if="result.composition_year"
      class="year"
      :title="`Composed ${result.composition_year}`"
      :aria-label="`Composed ${result.composition_year}`"
      >{{ result.composition_year }}</span
    >
  </RouterLink>
</template>

<script setup>
import "@/assets/search.css";
import { computed } from "vue";
import WorkArtwork from "@/components/common/WorkArtwork.vue";

const props = defineProps({ result: { type: Object, required: true } });
const creators = computed(() => (props.result.creators || []).join(", "));
</script>

<style scoped>
.workResult {
  text-decoration: none;
  box-sizing: border-box;
  min-width: 0;
}
.workResult:focus-visible {
  outline: 2px solid var(--accent-color);
  outline-offset: -2px;
}
.column {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-width: 0;
  margin-right: 8px;
}
.title {
  font-size: 16px;
  font-weight: bold;
}
.creators {
  font-size: 14px;
  color: var(--text-subdued);
}
.title,
.creators {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.year {
  flex-shrink: 0;
  font-size: 14px;
  font-variant-numeric: tabular-nums;
}
</style>
