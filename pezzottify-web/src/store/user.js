import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import axios from 'axios';

export const useUserStore = defineStore('user', () => {

  const likedAlbumIds = ref(null);
  let isLoadingLikedAlbums = ref(false);

  const loadLikedAlbumIds = async () => {
    isLoadingLikedAlbums.value = true;
    try {
      const response = await axios.get('/v1/user/liked/album');
      console.log("Writing new data to likedAlbumIds");
      console.log(response.data);
      likedAlbumIds.value = response.data;
    } catch (error) {
      console.error('Failed to load liked albums:', error);
    } finally {
      isLoadingLikedAlbums.value = false;
    }
  };

  const triggerAlbumsLoad = async () => {
    if (!likedAlbumIds.value && !isLoadingLikedAlbums.value) {
      await loadLikedAlbumIds();
    }
  };

  const setAlbumIsLiked = async (albumId, isLiked) => {
    try {
      if (isLiked) {
        await axios.post(`/v1/user/liked/${albumId}`);
      } else {
        await axios.delete(`/v1/user/liked/${albumId}`);
      }
      if (isLiked) {
        likedAlbumIds.value = [albumId, ...likedAlbumIds.value];
      } else {
        likedAlbumIds.value = likedAlbumIds.value.filter(id => id !== albumId);
      }
    } catch (error) {
      console.error('Failed to update liked status:', error);
    }
  }

  return {
    isLoadingLikedAlbums,
    likedAlbumIds,
    triggerAlbumsLoad,
    setAlbumIsLiked,
  };
});
