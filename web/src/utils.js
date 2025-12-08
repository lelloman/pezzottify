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

const makeImageIdsSortingFunction = (sortingPreferences, objectProps) => {
  return (targetObject) => {
    const mapImg = (preferred) => {
      return (img) => {
        return {
          id: img.id,
          size: img.size,
          preferred: preferred,
        };
      };
    };
    const allImages = targetObject[objectProps[0]]
      ? targetObject[objectProps[0]].map(mapImg(true))
      : [];
    allImages.push(...targetObject[objectProps[1]].map(mapImg(false)));
    function imageSizeValue(x) {
      return sortingPreferences[x.size] || 0;
    }
    const sortedImages = allImages.sort((a, b) => {
      if (a.preferred === b.preferred) {
        return imageSizeValue(b) - imageSizeValue(a);
      }
      return a.preferred ? -1 : 1;
    });
    return sortedImages.map((img) => img.id);
  };
};

const makeImageUrlsSortingFunction = (sortingPreferences, objectProps) => {
  return (targetObject) => {
    return makeImageIdsSortingFunction(
      sortingPreferences,
      objectProps,
    )(targetObject).map((imageId) => formatImageUrl(imageId));
  };
};

const bigImageSizePrefs = {
  XLARGE: 5,
  LARGE: 4,
  DEFAULT: 2,
  SMALL: 1,
};

const smallImageSizePrefs = {
  SMALL: 4,
  DEFAULT: 3,
  LARGE: 2,
  XLARGE: 1,
};

const artistProps = ["portrait_group", "portraits"];
const albumProps = ["covers", "cover_group"];

export const chooseArtistCoverImageUrl = makeImageUrlsSortingFunction(
  bigImageSizePrefs,
  artistProps,
);
export const chooseSmallArtistImageUrl = makeImageUrlsSortingFunction(
  smallImageSizePrefs,
  artistProps,
);
export const chooseAlbumCoverImageUrl = makeImageUrlsSortingFunction(
  bigImageSizePrefs,
  albumProps,
);
export const chooseAlbumCoverImageIds = makeImageIdsSortingFunction(
  smallImageSizePrefs,
  albumProps,
);
