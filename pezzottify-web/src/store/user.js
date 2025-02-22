import { defineStore } from 'pinia';
import { ref } from 'vue';
import axios from 'axios';

export const useUserStore = defineStore('user', () => {

  const likedAlbumIds = ref(null);
  let isLoadingLikedAlbums = ref(false);

  const likedArtistsIds = ref(null);
  let isLoadingLikedArtists = ref(false);

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

  const loadLikedArtistsIds = async () => {
    isLoadingLikedArtists.value = true;
    try {
      const response = await axios.get('/v1/user/liked/artist');
      console.log("Writing new data to likedArtistsIds");
      console.log(response.data);
      likedArtistsIds.value = response.data;
    } catch (error) {
      console.error('Failed to load liked artists:', error);
    } finally {
      isLoadingLikedArtists.value = false;
    }
  }

  const triggerArtistsLoad = async () => {
    if (!likedArtistsIds.value && !isLoadingLikedArtists.value) {
      await loadLikedArtistsIds();
    }
  }

  const setArtistIsLiked = async (artistId, isLiked) => {
    try {
      if (isLiked) {
        await axios.post(`/v1/user/liked/${artistId}`);
      } else {
        await axios.delete(`/v1/user/liked/${artistId}`);
      }
      if (isLiked) {
        likedArtistsIds.value = [artistId, ...likedArtistsIds.value];
      } else {
        likedArtistsIds.value = likedArtistsIds.value.filter(id => id !== artistId);
      }
    } catch (error) {
      console.error('Failed to update liked status:', error);
    }
  }

  return {
    isLoadingLikedAlbums,
    likedAlbumIds,
    isLoadingLikedArtists,
    likedArtistsIds,
    triggerAlbumsLoad,
    setAlbumIsLiked,
    triggerArtistsLoad,
    setArtistIsLiked,
  };
});
