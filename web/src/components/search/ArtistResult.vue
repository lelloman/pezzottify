<template>
  <div class="searchResultRow" :data-id="result" @click="handleClick(result)">
    <MultiSourceImage
      :urls="[imageUrl]"
      alt="Image"
      class="searchResultRoundImage"
    />
    <h3 class="title">{{ result.name }}</h3>
  </div>
</template>

<script setup>
import "@/assets/search.css";
import { computedImageUrl } from "@/utils";
import { useRouter } from "vue-router";
import MultiSourceImage from "../common/MultiSourceImage.vue";

const props = defineProps({
  result: {
    type: Object,
    required: true,
  },
});
const imageUrl = computedImageUrl(props.result.image_id);

const router = useRouter();

const handleClick = (event) => {
  router.push("/artist/" + event.id);
};
</script>

<style scoped>
.title {
  margin: 0;
  font-size: 16px;
  font-weight: bold;
}
</style>
