import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import { useRemoteStore } from './remote';

// Settings key constants
export const SETTING_ENABLE_DIRECT_DOWNLOADS = 'enable_direct_downloads';

export const useUserStore = defineStore('user', () => {
  const remoteStore = useRemoteStore();
  const likedAlbumIds = ref(null);
  const likedArtistsIds = ref(null);
  const playlistsData = ref(null);
  const playlistRefs = {};
  const settings = ref({});

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
      const [albumsData, artistsData, playlistsResponse, settingsData] = await Promise.all([
        remoteStore.fetchLikedAlbums(),
        remoteStore.fetchLikedArtists(),
        remoteStore.fetchUserPlaylists(),
        remoteStore.fetchUserSettings()
      ]);

      // Update state with fetched data
      likedAlbumIds.value = albumsData;
      likedArtistsIds.value = artistsData;
      settings.value = settingsData || {};

      const by_id = {};
      playlistsResponse.forEach(playlist => {
        by_id[playlist.id] = playlist;
      });

      playlistsData.value = {
        list: playlistsResponse,
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
    const success = await remoteStore.setAlbumLikeStatus(albumId, isLiked);
    if (success) {
      if (isLiked) {
        likedAlbumIds.value = [albumId, ...likedAlbumIds.value];
      } else {
        likedAlbumIds.value = likedAlbumIds.value.filter(id => id !== albumId);
      }
    }
  }

  const setArtistIsLiked = async (artistId, isLiked) => {
    const success = await remoteStore.setArtistLikeStatus(artistId, isLiked);
    if (success) {
      if (isLiked) {
        likedArtistsIds.value = [artistId, ...likedArtistsIds.value];
      } else {
        likedArtistsIds.value = likedArtistsIds.value.filter(id => id !== artistId);
      }
    }
  }

  const getPlaylistRef = (playlistId) => {
    if (!playlistRefs[playlistId]) {
      playlistRefs[playlistId] = {
        value: computed(() => {
          if (!playlistsData.value || !playlistsData.value.by_id) return null;
          return playlistsData.value.by_id[playlistId];
        }),
        refCount: 1,
      };
    } else {
      playlistRefs[playlistId].refCount++;
    }
    return playlistRefs[playlistId].value;
  };

  const putPlaylistRef = (playlistId) => {
    if (playlistRefs[playlistId]) {
      playlistRefs[playlistId].refCount--;
      if (playlistRefs[playlistId].refCount === 0) {
        delete playlistRefs[playlistId];
      }
    }
  };

  const loadPlaylistData = async (playlistId) => {
    if (playlistsData.value && playlistsData.value.by_id[playlistId]) {
      return;
    }

    const playlistData = await remoteStore.fetchPlaylistData(playlistId);
    if (playlistData && playlistsData.value) {
      playlistsData.value.list = playlistsData.value.list.map(playlist => {
        if (playlist.id === playlistId) {
          return playlistData;
        }
        return playlist;
      });
      playlistsData.value.by_id[playlistId] = playlistData;
    }
  }

  const createPlaylist = async (callback) => {
    const newPlaylist = await remoteStore.createNewPlaylist();
    if (newPlaylist && playlistsData.value) {
      console.log("Creating new playlist");
      console.log(newPlaylist);
      playlistsData.value.list = [newPlaylist, ...playlistsData.value.list];
      playlistsData.value.by_id[newPlaylist.id] = newPlaylist;
    }
    callback(newPlaylist);
  }

  const deletePlaylist = async (playlistId, callback) => {
    const success = await remoteStore.deleteUserPlaylist(playlistId);
    if (success && playlistsData.value) {
      const oldValue = playlistsData.value;
      delete oldValue.by_id[playlistId];
      oldValue.list = oldValue.list.filter(playlist => playlist !== playlistId);
      playlistsData.value = oldValue;
      if (playlistRefs[playlistId]) {
        delete playlistRefs[playlistId];
      }
    }
    callback(success);
  }

  const updatePlaylistName = async (playlistId, name, callback) => {
    const success = await remoteStore.updatePlaylistName(playlistId, name);
    if (success && playlistsData.value && playlistsData.value.by_id[playlistId]) {
      // Update name in memory
      playlistsData.value.by_id[playlistId].name = name;

      // Update the playlist in the list
      playlistsData.value.list = playlistsData.value.list.map(p =>
        p.id === playlistId ? { ...p, name } : p
      );
    }
    callback(success);
  }

  const addTracksToPlaylist = async (playlistId, trackIds, callback) => {
    const success = await remoteStore.addTracksToPlaylist(playlistId, trackIds);
    console.log("user store addTracksToPlaylist success: " + success);
    if (success && playlistsData.value && playlistsData.value.by_id[playlistId]) {
      const playlist = playlistsData.value.by_id[playlistId];
      playlist.tracks = [...playlist.tracks, ...trackIds];
      console.log("user store addTracksToPlaylist playlist:");

    }
    callback(success);
  }

  const removeTracksFromPlaylist = async (playlistId, tracksPositions, callback) => {
    const success = await remoteStore.removeTracksFromPlaylist(playlistId, tracksPositions);
    if (success && playlistsData.value && playlistsData.value.by_id[playlistId]) {
      const playlist = playlistsData.value.by_id[playlistId];

      const newTracks = [];
      playlist.tracks.forEach((trackId, index) => {
        if (!tracksPositions.includes(index)) {
          newTracks.push(trackId);
        }
      });
      playlist.tracks = newTracks;
    }
    callback(success);
  }

  // Settings methods
  const getSetting = (key) => {
    return settings.value[key];
  };

  const setSetting = async (key, value) => {
    const success = await remoteStore.updateUserSettings({ [key]: value });
    if (success) {
      settings.value = { ...settings.value, [key]: value };
    }
    return success;
  };

  // Convenience computed for direct downloads setting
  const isDirectDownloadsEnabled = computed(() => {
    return settings.value[SETTING_ENABLE_DIRECT_DOWNLOADS] === 'true';
  });

  const setDirectDownloadsEnabled = async (enabled) => {
    return await setSetting(SETTING_ENABLE_DIRECT_DOWNLOADS, enabled ? 'true' : 'false');
  };

  return {
    likedAlbumIds,
    likedArtistsIds,
    playlistsData,
    settings,
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
    removeTracksFromPlaylist,
    getPlaylistRef,
    putPlaylistRef,
    getSetting,
    setSetting,
    isDirectDownloadsEnabled,
    setDirectDownloadsEnabled,
  };
});
