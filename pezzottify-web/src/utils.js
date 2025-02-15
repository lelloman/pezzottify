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

const makeImageUrlSortingFunction = (sortingPreferences) => {
  return (artist) => {
    const mapImg = (preferred) => {
      return (img) => {
        return {
          id: img.id,
          size: img.size,
          preferred: preferred,
        }
      };
    };
    const allImages = artist.portrait_group ? artist.portrait_group.map(mapImg(true)) : [];
    allImages.push(...artist.portraits.map(mapImg(false)));
    function imageSizeValue(x) {
      return sortingPreferences[x.size] || 0;
    }
    const sortedImages = allImages.sort((a, b) => {
      if (a.preferred === b.preferred) {
        return imageSizeValue(b) - imageSizeValue(a);
      }
      return a.preferred ? -1 : 1;
    });
    return sortedImages.map((img) => formatImageUrl(img.id));
  }
}

const artistCoverImageSizePreferences = {
  "XLARGE": 5,
  "LARGE": 4,
  "DEFAULT": 2,
  "SMALL": 1,
}

const artistSmallImageSizePreferences = {
  "SMALL": 4,
  "DEFAULT": 3,
  "LARGE": 2,
  "XLARGE": 1,
}

export const chooseCoverImageUrl = makeImageUrlSortingFunction(artistCoverImageSizePreferences);
export const chooseSmallArtistImageUrl = makeImageUrlSortingFunction(artistSmallImageSizePreferences);
