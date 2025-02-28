import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import axios from 'axios';

const STORAGE_KEY = 'pezzottify-user-data';

export const useUserStore = defineStore('user', () => {
  const likedAlbumIds = ref(null);

  const likedArtistsIds = ref(null);

  const playlistsData = ref(null);
  const playlistRefs = {};

  // New overall loading state
  const isInitialized = ref(false);
  const isInitializing = ref(false);

  // Load data from localStorage on store creation
  const loadFromStorage = () => {
    try {
      const storedData = localStorage.getItem(STORAGE_KEY);
      if (storedData) {
        const parsedData = JSON.parse(storedData);
        if (parsedData.likedAlbumIds) likedAlbumIds.value = parsedData.likedAlbumIds;
        if (parsedData.likedArtistsIds) likedArtistsIds.value = parsedData.likedArtistsIds;
        if (parsedData.playlistsData) playlistsData.value = parsedData.playlistsData;
        console.log('Loaded user data from localStorage');
        return true;
      }
    } catch (error) {
      console.error('Failed to load data from localStorage:', error);
    }
    return false;
  };

  // Save current state to localStorage
  const saveToStorage = () => {
    try {
      const dataToSave = {
        likedAlbumIds: likedAlbumIds.value,
        likedArtistsIds: likedArtistsIds.value,
        playlistsData: playlistsData.value,
      };
      localStorage.setItem(STORAGE_KEY, JSON.stringify(dataToSave));
    } catch (error) {
      console.error('Failed to save data to localStorage:', error);
    }
  };

  // Load all user data
  const initialize = async (forceRefresh = false) => {
    // Return early if already initialized and not forcing refresh
    if (isInitialized.value && !forceRefresh) return true;

    // Return early if already initializing
    if (isInitializing.value) return false;

    // Try loading from localStorage first if not forcing refresh
    if (!forceRefresh && loadFromStorage()) {
      isInitialized.value = true;
      return true;
    }

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

      // Save to localStorage
      saveToStorage();

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
      saveToStorage();
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
      saveToStorage();
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
        saveToStorage();
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
        saveToStorage();
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
        playlistsData.value.by_id[playlistId] = null;
        saveToStorage();
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
        playlistsData.value.by_id[playlistId].name = name;
        saveToStorage();
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
        saveToStorage();
      }
      callback(true);
    } catch (error) {
      console.error('Failed to add tracks to playlist:', error);
      callback(false);
    }
  }

  // Try to load from localStorage immediately when store is created
  loadFromStorage();

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
