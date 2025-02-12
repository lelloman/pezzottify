import { computed } from 'vue';

export const computeImageUrl = (image_id) => computed(() => {
  return 'v1/content/image/' + image_id;
});
