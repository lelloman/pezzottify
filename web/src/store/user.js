import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import axios from 'axios';


export const useUserStore = defineStore('user', () => {
  const likedAlbumIds = ref(null);
  const likedArtistsIds = ref(null);
  const playlistsData = ref(null);
  const playlistRefs = {};

  const isInitialized = ref(false);
  const isInitializing = ref(false);

  // Load all user data
  const initialize = async () => {
    // Return early if already initialized and not forcing refresh
    if (isInitialized.value) return true;

    // Return early if already initializing
    if (isInitializing.value) return false;

    isInitializing.value = true;

    try {
      // Load all data in parallel
      const [albumsResponse, artistsResponse, playlistsResponse] = await Promise.all([
        axios.get('/v1/user/liked/album').catch(error => {
          console.error('Failed to load liked albums:', error);
          return { data: [] };
        }),
        axios.get('/v1/user/liked/artist').catch(error => {
          console.error('Failed to load liked artists:', error);
          return { data: [] };
        }),
        axios.get('/v1/user/playlists').catch(error => {
          console.error('Failed to load playlists:', error);
          return { data: [] };
        })
      ]);

      // Update state with fetched data
      likedAlbumIds.value = albumsResponse.data;
      likedArtistsIds.value = artistsResponse.data;

      const by_id = {};
      playlistsResponse.data.forEach(playlist => {
        by_id[playlist.id] = playlist;
      });

      playlistsData.value = {
        list: playlistsResponse.data,
        by_id: by_id,
      };

      isInitialized.value = true;
      return true;
    } catch (error) {
      console.error('Failed to initialize user data:', error);
      return false;
    } finally {
      isInitializing.value = false;
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

  const getPlaylistRef = (playlistId) => {
    if (!playlistRefs[playlistId]) {
      playlistRefs[playlistId] = computed(() => {
        if (!playlistsData.value || !playlistsData.value.by_id) return null;
        return playlistsData.value.by_id[playlistId];
      });
    }
    return playlistRefs[playlistId];
  };

  const loadPlaylistData = async (playlistId) => {
    console.log("userStore loadPlaylistData playlistId: " + playlistId);
    if (playlistsData.value && playlistsData.value.by_id[playlistId]) {
      return;
    }
    try {
      const response = await axios.get(`/v1/user/playlist/${playlistId}`);
      console.log("Writing new data to playlistData");
      console.log(response.data);
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
      if (playlistsData.value) {
        playlistsData.value.list = [response.data, ...playlistsData.value.list];
        playlistsData.value.by_id[response.data.id] = response.data;
      }
      callback(response.data);
    } catch (error) {
      console.error('Failed to create new playlist:', error);
      callback(null);
    }
  }

  const deletePlaylist = async (playlistId, callback) => {
    try {
      await axios.delete(`/v1/user/playlist/${playlistId}`);
      if (playlistsData.value) {
        playlistsData.value.list = playlistsData.value.list.filter(playlist => playlist.id !== playlistId);
        delete playlistsData.value.by_id[playlistId];
      }
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
      if (playlistsData.value && playlistsData.value.by_id[playlistId]) {
        // Update name in memory
        playlistsData.value.by_id[playlistId].name = name;

        // Update the playlist in the list
        playlistsData.value.list = playlistsData.value.list.map(p =>
          p.id === playlistId ? { ...p, name } : p
        );
      }
      callback(true);
    } catch (error) {
      console.error('Failed to update playlist name:', error);
      callback(false);
    }
  }

  const addTracksToPlaylist = async (playlistId, trackIds, callback) => {
    try {
      await axios.put(`/v1/user/playlist/${playlistId}/add`, {
        tracks_ids: trackIds
      });
      if (playlistsData.value && playlistsData.value.by_id[playlistId]) {
        const playlist = playlistsData.value.by_id[playlistId];
        playlist.track_ids = [...playlist.tracks, ...trackIds];

        // update any refs if exists
        if (playlistRefs[playlistId]) {
          playlistRefs[playlistId].value = playlist;
        }
      }
      callback(true);
    } catch (error) {
      console.error('Failed to add tracks to playlist:', error);
      callback(false);
    }
  }

  return {
    likedAlbumIds,
    likedArtistsIds,
    playlistsData,
    isInitialized,
    isInitializing,
    initialize,
    setAlbumIsLiked,
    setArtistIsLiked,
    createPlaylist,
    deletePlaylist,
    loadPlaylistData,
    updatePlaylistName,
    addTracksToPlaylist,
    getPlaylistRef,
  };
});
