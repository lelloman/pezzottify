import { computed } from 'vue';

export function formatImageUrl(image_id) {
  return '/v1/content/image/' + image_id;
}

export const computedImageUrl = (image_id) => computed(() => {
  return formatImageUrl(image_id);
});

export function formatDuration(d) {
  const seconds = Math.round(d / 1000);
  const pad = (num) => String(num).padStart(2, '0');
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const secs = seconds % 60;
  return `${pad(hours)}:${pad(minutes)}:${pad(secs)}`;
}
