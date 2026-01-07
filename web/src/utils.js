import { computed } from "vue";

export function formatImageUrl(image_id) {
  return "/v1/content/image/" + image_id;
}

export const computedImageUrl = (image_id) =>
  computed(() => {
    return formatImageUrl(image_id);
  });

export function formatDuration(d) {
  const seconds = Math.round(d / 1000);
  const pad = (num) => String(num).padStart(2, "0");
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const secs = seconds % 60;
  return `${pad(hours)}:${pad(minutes)}:${pad(secs)}`;
}

export const getYearFromTimestamp = (unixTimestamp) =>
  new Date(unixTimestamp * 1000).getFullYear();

// Image endpoint now takes item IDs (album or artist ID) directly.
// The server lazily downloads and caches images from external URLs.

export const chooseArtistCoverImageUrl = (artist) => {
  if (!artist || !artist.id) return [];
  return [formatImageUrl(artist.id)];
};

export const chooseSmallArtistImageUrl = chooseArtistCoverImageUrl;

export const chooseAlbumCoverImageUrl = (album) => {
  if (!album || !album.id) return [];
  return [formatImageUrl(album.id)];
};

export const chooseAlbumCoverImageIds = (album) => {
  if (!album || !album.id) return [];
  return [album.id];
};
