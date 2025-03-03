import { defineStore } from 'pinia';
import axios from 'axios';

export const useRemoteStore = defineStore('remote', () => {

  const setBlockHttpCache = (value) => {
    if (value) {
      axios.defaults.headers.common['Cache-Control'] = 'no-cache, no-store, must-revalidate';
      axios.defaults.headers.common['Pragma'] = 'no-cache';
      axios.defaults.headers.common['Expires'] = '0';
    } else {
      delete axios.defaults.headers.common['Cache-Control'];
      delete axios.defaults.headers.common['Pragma'];
      delete axios.defaults.headers.common['Expires'];
    }
  }

  // User data fetching
  const fetchLikedAlbums = async () => {
    try {
      const response = await axios.get('/v1/user/liked/album');
      return response.data;
    } catch (error) {
      console.error('Failed to load liked albums:', error);
      return [];
    }
  };

  const fetchLikedArtists = async () => {
    try {
      const response = await axios.get('/v1/user/liked/artist');
      return response.data;
    } catch (error) {
      console.error('Failed to load liked artists:', error);
      return [];
    }
  };

  const fetchUserPlaylists = async () => {
    try {
      const response = await axios.get('/v1/user/playlists');
      return response.data;
    } catch (error) {
      console.error('Failed to load playlists:', error);
      return [];
    }
  };

  // Like/unlike operations
  const setAlbumLikeStatus = async (albumId, isLiked) => {
    try {
      if (isLiked) {
        await axios.post(`/v1/user/liked/${albumId}`);
      } else {
        await axios.delete(`/v1/user/liked/${albumId}`);
      }
      return true;
    } catch (error) {
      console.error('Failed to update album liked status:', error);
      return false;
    }
  };

  const setArtistLikeStatus = async (artistId, isLiked) => {
    try {
      if (isLiked) {
        await axios.post(`/v1/user/liked/${artistId}`);
      } else {
        await axios.delete(`/v1/user/liked/${artistId}`);
      }
      return true;
    } catch (error) {
      console.error('Failed to update artist liked status:', error);
      return false;
    }
  };

  // Playlist operations
  const fetchPlaylistData = async (playlistId) => {
    try {
      const response = await axios.get(`/v1/user/playlist/${playlistId}`);
      return response.data;
    } catch (error) {
      console.error('Failed to load playlist data:', error);
      return null;
    }
  };

  const createNewPlaylist = async () => {
    try {
      const response = await axios.post('/v1/user/playlist', {
        name: 'New Playlist',
        track_ids: [],
      });
      return response.data;
    } catch (error) {
      console.error('Failed to create new playlist:', error);
      return null;
    }
  };

  const deleteUserPlaylist = async (playlistId) => {
    try {
      await axios.delete(`/v1/user/playlist/${playlistId}`);
      return true;
    } catch (error) {
      console.error('Failed to delete playlist:', error);
      return false;
    }
  };

  const updatePlaylistName = async (playlistId, name) => {
    try {
      await axios.put(`/v1/user/playlist/${playlistId}`, {
        name: name,
      });
      return true;
    } catch (error) {
      console.error('Failed to update playlist name:', error);
      return false;
    }
  };

  const addTracksToPlaylist = async (playlistId, trackIds) => {
    try {
      await axios.put(`/v1/user/playlist/${playlistId}/add`, {
        tracks_ids: trackIds
      });
      return true;
    } catch (error) {
      console.error('Failed to add tracks to playlist:', error);
      return false;
    }
  };

  const removeTracksFromPlaylist = async (playlistId, tracksPositions) => {
    try {
      await axios.put(`/v1/user/playlist/${playlistId}/remove`, {
        tracks_positions: tracksPositions
      });
      return true;
    } catch (error) {
      console.error('Failed to remove tracks from playlist:', error);
      return false;
    }
  }

  // Track operations
  const fetchTrack = async (trackId) => {
    try {
      const response = await axios.get(`/v1/content/track/${trackId}`);
      return response.data;
    } catch (error) {
      console.error('Error fetching track data:', error);
      return null;
    }
  };

  const fetchResolvedTrack = async (trackId) => {
    try {
      const response = await axios.get(`/v1/content/track/${trackId}/resolved`);
      return response.data;
    } catch (error) {
      console.error('Error fetching resolved track data:', error);
      return null;
    }
  };

  // Album operations
  const fetchResolvedAlbum = async (albumId) => {
    try {
      const response = await axios.get(`/v1/content/album/${albumId}/resolved`);
      return response.data;
    } catch (error) {
      console.error('Error fetching album data:', error);
      return null;
    }
  };

  const fetchAlbum = async (albumId) => {
    try {
      const response = await axios.get(`/v1/content/album/${albumId}`);
      return response.data;
    } catch (error) {
      console.error('Error fetching album data:', error);
      return null;
    }
  };

  // Artist operations
  const fetchArtist = async (artistId) => {
    try {
      const response = await axios.get(`/v1/content/artist/${artistId}`);
      return response.data;
    } catch (error) {
      console.error('Error fetching artist data:', error);
      return null;
    }
  };

  const fetchArtistAlbums = async (artistId) => {
    try {
      const response = await axios.get(`/v1/content/artist/${artistId}/albums`);
      return response.data;
    } catch (error) {
      console.error('Error fetching artist albums:', error);
      return [];
    }
  };

  return {
    setBlockHttpCache,
    fetchLikedAlbums,
    fetchLikedArtists,
    fetchUserPlaylists,
    setAlbumLikeStatus,
    setArtistLikeStatus,
    fetchPlaylistData,
    createNewPlaylist,
    deleteUserPlaylist,
    updatePlaylistName,
    addTracksToPlaylist,
    removeTracksFromPlaylist,
    fetchTrack,
    fetchResolvedTrack,
    fetchResolvedAlbum,
    fetchAlbum,
    fetchArtist,
    fetchArtistAlbums,
  };
});
