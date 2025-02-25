import { defineStore } from 'pinia';
import { ref } from 'vue';
import axios from 'axios';

export const useUserStore = defineStore('user', () => {

  const likedAlbumIds = ref(null);
  let isLoadingLikedAlbums = ref(false);

  const likedArtistsIds = ref(null);
  let isLoadingLikedArtists = ref(false);

  const playlistsData = ref(null);
  let isLoadingPlaylists = ref(false);


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

  const loadPlaylists = async () => {
    isLoadingPlaylists.value = true;
    try {
      const response = await axios.get('/v1/user/playlists');
      console.log("Writing new data to playlistsIds");
      console.log(response.data);

      const by_id = {};
      response.data.forEach(playlist => {
        by_id[playlist.id] = playlist.id;
      });

      playlistsData.value = {
        list: response.data,
        by_id: by_id,
      }
    } catch (error) {
      console.error('Failed to load playlists:', error);
    } finally {
      isLoadingPlaylists.value = false;
    }
  }

  const triggerPlaylistsLoad = async () => {
    if (playlistsData.value == null && !isLoadingPlaylists.value) {
      await loadPlaylists();
    }
  }

  const loadPlaylistData = async (playlistId, callback) => {
    console.log("userStore loadPlaylistData playlistId: " + playlistId);
    if (playlistsData.value && playlistsData.value.by_id[playlistId]) {
      const mapped = playlistsData.value.by_id[playlistId];
      if (mapped) {
        callback(mapped);
        return;
      }
    }
    try {
      const response = await axios.get(`/v1/user/playlist/${playlistId}`);
      console.log("Writing new data to playlistData");
      console.log(response.data);
      callback(response.data);
      if (playlistsData.value) {
        playlistsData.value.list = playlistsData.value.list.map(playlist => {
          if (playlist.id === playlistId) {
            return response.data;
          }
          return playlist;
        });
        playlistsData.value.by_id[playlistId] = response.data;
      }

    } catch (error) {
      console.error('Failed to load playlist data:', error);
    }
  }

  const createPlaylist = async (callback) => {
    try {
      const response = await axios.post('/v1/user/playlist', {
        name: 'New Playlist',
        track_ids: [],
      });
      console.log("Creating new playlist");
      console.log(response.data);
      playlistsData.value.list = [response.data, ...playlistsData.value.list];
      playlistsData.value.by_id[response.data.id] = response.data;
      callback(response.data);
    } catch (error) {
      console.error('Failed to create new playlist:', error);
      callback(null);
    }
  }

  const deletePlaylist = async (playlistId, callback) => {
    try {
      await axios.delete(`/v1/user/playlist/${playlistId}`);
      playlistsData.value.list = playlistsData.value.list.filter(id => id !== playlistId);
      playlistsData.value.by_id[playlistId] = null;
      callback(true);
    } catch (error) {
      console.error('Failed to delete playlist:', error);
      callback(false);
    }
  }

  const updatePlaylistName = async (playlistId, name, callback) => {
    try {
      await axios.put(`/v1/user/playlist/${playlistId}`, {
        name: name,
      });
      playlistsData.value.by_id[playlistId].name = name;
      callback(true);
    } catch (error) {
      console.error('Failed to update playlist name:', error);
      callback(false);
    }
  }

  return {
    isLoadingLikedAlbums,
    likedAlbumIds,
    isLoadingLikedArtists,
    likedArtistsIds,
    isLoadingPlaylists,
    playlistsData,
    triggerAlbumsLoad,
    setAlbumIsLiked,
    triggerArtistsLoad,
    setArtistIsLiked,
    triggerPlaylistsLoad,
    createPlaylist,
    deletePlaylist,
    loadPlaylistData,
    updatePlaylistName,
  };
});
