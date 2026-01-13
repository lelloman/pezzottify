import { defineStore } from "pinia";
import axios from "axios";

export const useRemoteStore = defineStore("remote", () => {
  const setBlockHttpCache = (value) => {
    if (value) {
      axios.defaults.headers.common["Cache-Control"] =
        "no-cache, no-store, must-revalidate";
      axios.defaults.headers.common["Pragma"] = "no-cache";
      axios.defaults.headers.common["Expires"] = "0";
    } else {
      delete axios.defaults.headers.common["Cache-Control"];
      delete axios.defaults.headers.common["Pragma"];
      delete axios.defaults.headers.common["Expires"];
    }
  };

  // User data fetching
  const fetchLikedAlbums = async () => {
    try {
      const response = await axios.get("/v1/user/liked/album");
      return response.data;
    } catch (error) {
      console.error("Failed to load liked albums:", error);
      return [];
    }
  };

  const fetchLikedArtists = async () => {
    try {
      const response = await axios.get("/v1/user/liked/artist");
      return response.data;
    } catch (error) {
      console.error("Failed to load liked artists:", error);
      return [];
    }
  };

  const fetchUserPlaylists = async () => {
    try {
      const response = await axios.get("/v1/user/playlists");
      return response.data;
    } catch (error) {
      console.error("Failed to load playlists:", error);
      return [];
    }
  };

  // Like/unlike operations
  const setAlbumLikeStatus = async (albumId, isLiked) => {
    try {
      if (isLiked) {
        await axios.post(`/v1/user/liked/album/${albumId}`);
      } else {
        await axios.delete(`/v1/user/liked/album/${albumId}`);
      }
      return true;
    } catch (error) {
      console.error("Failed to update album liked status:", error);
      return false;
    }
  };

  const setArtistLikeStatus = async (artistId, isLiked) => {
    try {
      if (isLiked) {
        await axios.post(`/v1/user/liked/artist/${artistId}`);
      } else {
        await axios.delete(`/v1/user/liked/artist/${artistId}`);
      }
      return true;
    } catch (error) {
      console.error("Failed to update artist liked status:", error);
      return false;
    }
  };

  // Playlist operations
  const fetchPlaylistData = async (playlistId) => {
    try {
      const response = await axios.get(`/v1/user/playlist/${playlistId}`);
      return response.data;
    } catch (error) {
      console.error("Failed to load playlist data:", error);
      return null;
    }
  };

  const createNewPlaylist = async () => {
    try {
      const response = await axios.post("/v1/user/playlist", {
        name: "New Playlist",
        track_ids: [],
      });
      return response.data;
    } catch (error) {
      console.error("Failed to create new playlist:", error);
      return null;
    }
  };

  const deleteUserPlaylist = async (playlistId) => {
    try {
      await axios.delete(`/v1/user/playlist/${playlistId}`);
      return true;
    } catch (error) {
      console.error("Failed to delete playlist:", error);
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
      console.error("Failed to update playlist name:", error);
      return false;
    }
  };

  const addTracksToPlaylist = async (playlistId, trackIds) => {
    try {
      await axios.put(`/v1/user/playlist/${playlistId}/add`, {
        tracks_ids: trackIds,
      });
      return true;
    } catch (error) {
      console.error("Failed to add tracks to playlist:", error);
      return false;
    }
  };

  const removeTracksFromPlaylist = async (playlistId, tracksPositions) => {
    try {
      await axios.put(`/v1/user/playlist/${playlistId}/remove`, {
        tracks_positions: tracksPositions,
      });
      return true;
    } catch (error) {
      console.error("Failed to remove tracks from playlist:", error);
      return false;
    }
  };

  // User settings operations
  const fetchUserSettings = async () => {
    try {
      const response = await axios.get("/v1/user/settings");
      return response.data.settings;
    } catch (error) {
      console.error("Failed to fetch user settings:", error);
      return {};
    }
  };

  const updateUserSettings = async (settings) => {
    try {
      // Convert from { key: value } format to server's expected format:
      // { settings: [{ key: "setting_key", value: settingValue }] }
      const settingsArray = Object.entries(settings).map(([key, value]) => ({
        key,
        value: value === "true" ? true : value === "false" ? false : value,
      }));
      await axios.put("/v1/user/settings", { settings: settingsArray });
      return true;
    } catch (error) {
      console.error("Failed to update user settings:", error);
      return false;
    }
  };

  // Track operations
  const fetchTrack = async (trackId) => {
    try {
      const response = await axios.get(`/v1/content/track/${trackId}`);
      return response.data;
    } catch (error) {
      console.error("Error fetching track data:", error);
      return null;
    }
  };

  const fetchResolvedTrack = async (trackId) => {
    try {
      const response = await axios.get(`/v1/content/track/${trackId}/resolved`);
      return response.data;
    } catch (error) {
      console.error("Error fetching resolved track data:", error);
      return null;
    }
  };

  // Album operations
  const fetchResolvedAlbum = async (albumId) => {
    try {
      const response = await axios.get(`/v1/content/album/${albumId}/resolved`);
      return response.data;
    } catch (error) {
      console.error("Error fetching album data:", error);
      return null;
    }
  };

  const fetchAlbum = async (albumId) => {
    try {
      const response = await axios.get(`/v1/content/album/${albumId}`);
      return response.data;
    } catch (error) {
      console.error("Error fetching album data:", error);
      return null;
    }
  };

  // Artist operations
  const fetchArtist = async (artistId) => {
    try {
      const response = await axios.get(`/v1/content/artist/${artistId}`);
      return response.data;
    } catch (error) {
      console.error("Error fetching artist data:", error);
      return null;
    }
  };

  const fetchArtistDiscography = async (
    artistId,
    { limit = 50, offset = 0, sort = "popularity" } = {},
  ) => {
    try {
      const response = await axios.get(
        `/v1/content/artist/${artistId}/discography`,
        { params: { limit, offset, sort } },
      );
      return response.data;
    } catch (error) {
      console.error("Error fetching artist albums:", error);
      return null;
    }
  };

  // Genre operations
  const fetchGenres = async () => {
    try {
      const response = await axios.get("/v1/content/genres");
      return response.data;
    } catch (error) {
      console.error("Error fetching genres:", error);
      return [];
    }
  };

  const fetchGenreTracks = async (genreName, limit = 20, offset = 0) => {
    try {
      const response = await axios.get(
        `/v1/content/genre/${encodeURIComponent(genreName)}/tracks`,
        { params: { limit, offset } },
      );
      return response.data;
    } catch (error) {
      console.error("Error fetching genre tracks:", error);
      return null;
    }
  };

  const fetchGenreRadio = async (genreName, count = 50) => {
    try {
      const response = await axios.get(
        `/v1/content/genre/${encodeURIComponent(genreName)}/radio`,
        { params: { count } },
      );
      return response.data;
    } catch (error) {
      console.error("Error fetching genre radio:", error);
      return [];
    }
  };

  // Impression tracking (for popularity scoring)
  const recordImpression = async (itemType, itemId) => {
    try {
      await axios.post("/v1/user/impression", {
        item_type: itemType,
        item_id: itemId,
      });
    } catch (error) {
      // Silent fail - impression tracking is non-critical
      console.debug("Failed to record impression:", error);
    }
  };

  // Sync API operations
  const fetchSyncState = async () => {
    try {
      const response = await axios.get("/v1/sync/state");
      return response.data;
    } catch (error) {
      console.error("Error fetching sync state:", error);
      throw error;
    }
  };

  const fetchSyncEvents = async (since) => {
    try {
      const response = await axios.get("/v1/sync/events", {
        params: { since },
      });
      return response.data;
    } catch (error) {
      // Return error info for 410 Gone handling
      if (error.response && error.response.status === 410) {
        return { error: "events_pruned", status: 410 };
      }
      console.error("Error fetching sync events:", error);
      throw error;
    }
  };

  // =====================================================
  // Admin API - User Management (ManagePermissions)
  // =====================================================

  const fetchAdminUsers = async () => {
    try {
      console.log("Fetching admin users...");
      const response = await axios.get("/v1/admin/users");
      console.log("Admin users response:", response.data);
      return response.data;
    } catch (error) {
      console.error("Failed to fetch users:", error);
      if (error.response) {
        console.error("Response status:", error.response.status);
        console.error("Response data:", error.response.data);
      }
      return null;
    }
  };

  const createUser = async (userHandle) => {
    try {
      const response = await axios.post("/v1/admin/users", {
        user_handle: userHandle,
      });
      return response.data;
    } catch (error) {
      console.error("Failed to create user:", error);
      if (error.response?.status === 409) {
        return { error: "User handle already exists" };
      }
      return { error: "Failed to create user" };
    }
  };

  const deleteUser = async (userHandle) => {
    try {
      await axios.delete(`/v1/admin/users/${userHandle}`);
      return { success: true };
    } catch (error) {
      console.error("Failed to delete user:", error);
      if (error.response?.status === 400) {
        return { error: "Cannot delete your own account" };
      }
      return { error: "Failed to delete user" };
    }
  };

  const fetchUserRoles = async (userHandle) => {
    try {
      const response = await axios.get(`/v1/admin/users/${userHandle}/roles`);
      return response.data;
    } catch (error) {
      console.error("Failed to fetch user roles:", error);
      return null;
    }
  };

  const addUserRole = async (userHandle, role) => {
    try {
      await axios.post(`/v1/admin/users/${userHandle}/roles`, { role });
      return true;
    } catch (error) {
      console.error("Failed to add user role:", error);
      return false;
    }
  };

  const removeUserRole = async (userHandle, role) => {
    try {
      await axios.delete(`/v1/admin/users/${userHandle}/roles/${role}`);
      return true;
    } catch (error) {
      console.error("Failed to remove user role:", error);
      return false;
    }
  };

  const fetchUserPermissions = async (userHandle) => {
    try {
      const response = await axios.get(
        `/v1/admin/users/${userHandle}/permissions`,
      );
      return response.data;
    } catch (error) {
      console.error("Failed to fetch user permissions:", error);
      return null;
    }
  };

  const grantPermission = async (
    userHandle,
    permission,
    durationSeconds = null,
    countdown = null,
  ) => {
    try {
      const body = { permission };
      if (durationSeconds !== null) body.duration_seconds = durationSeconds;
      if (countdown !== null) body.countdown = countdown;
      const response = await axios.post(
        `/v1/admin/users/${userHandle}/permissions`,
        body,
      );
      return response.data;
    } catch (error) {
      console.error("Failed to grant permission:", error);
      return null;
    }
  };

  const revokePermission = async (permissionId) => {
    try {
      await axios.delete(`/v1/admin/permissions/${permissionId}`);
      return true;
    } catch (error) {
      console.error("Failed to revoke permission:", error);
      return false;
    }
  };

  const fetchUserCredentialsStatus = async (userHandle) => {
    try {
      const response = await axios.get(
        `/v1/admin/users/${userHandle}/credentials`,
      );
      return response.data;
    } catch (error) {
      console.error("Failed to fetch user credentials status:", error);
      return null;
    }
  };

  const setUserPassword = async (userHandle, password) => {
    try {
      await axios.put(`/v1/admin/users/${userHandle}/password`, { password });
      return { success: true };
    } catch (error) {
      console.error("Failed to set user password:", error);
      return { error: "Failed to set password" };
    }
  };

  const deleteUserPassword = async (userHandle) => {
    try {
      await axios.delete(`/v1/admin/users/${userHandle}/password`);
      return { success: true };
    } catch (error) {
      console.error("Failed to delete user password:", error);
      return { error: "Failed to delete password" };
    }
  };

  // =====================================================
  // Admin API - Analytics (ViewAnalytics)
  // =====================================================

  const fetchDailyListening = async (startDate = null, endDate = null) => {
    try {
      const params = {};
      if (startDate) params.start_date = startDate;
      if (endDate) params.end_date = endDate;
      const response = await axios.get("/v1/admin/listening/daily", { params });
      return response.data;
    } catch (error) {
      console.error("Failed to fetch daily listening stats:", error);
      return null;
    }
  };

  const fetchTopTracks = async (
    startDate = null,
    endDate = null,
    limit = 50,
  ) => {
    try {
      const params = { limit };
      if (startDate) params.start_date = startDate;
      if (endDate) params.end_date = endDate;
      const response = await axios.get("/v1/admin/listening/top-tracks", {
        params,
      });
      return response.data;
    } catch (error) {
      console.error("Failed to fetch top tracks:", error);
      return null;
    }
  };

  const fetchTrackStats = async (trackId, startDate = null, endDate = null) => {
    try {
      const params = {};
      if (startDate) params.start_date = startDate;
      if (endDate) params.end_date = endDate;
      const response = await axios.get(`/v1/admin/listening/track/${trackId}`, {
        params,
      });
      return response.data;
    } catch (error) {
      console.error("Failed to fetch track stats:", error);
      return null;
    }
  };

  const fetchUserListeningSummary = async (
    userHandle,
    startDate = null,
    endDate = null,
  ) => {
    try {
      const params = {};
      if (startDate) params.start_date = startDate;
      if (endDate) params.end_date = endDate;
      const response = await axios.get(
        `/v1/admin/listening/users/${userHandle}/summary`,
        { params },
      );
      return response.data;
    } catch (error) {
      console.error("Failed to fetch user listening summary:", error);
      return null;
    }
  };

  const fetchOnlineUsers = async () => {
    try {
      const response = await axios.get("/v1/admin/online-users");
      return response.data;
    } catch (error) {
      console.error("Failed to fetch online users:", error);
      return null;
    }
  };

  // =====================================================
  // Admin API - Server Control (ServerAdmin)
  // =====================================================

  const rebootServer = async () => {
    try {
      await axios.post("/v1/admin/reboot");
      return true;
    } catch (error) {
      console.error("Failed to reboot server:", error);
      return false;
    }
  };

  const fetchBackgroundJobs = async () => {
    try {
      const response = await axios.get("/v1/admin/jobs");
      return response.data.jobs;
    } catch (error) {
      console.error("Failed to fetch background jobs:", error);
      return null;
    }
  };

  const triggerBackgroundJob = async (jobId, params = null) => {
    try {
      const body = params ? { params } : {};
      const response = await axios.post(`/v1/admin/jobs/${jobId}/trigger`, body);
      return { success: true, data: response.data };
    } catch (error) {
      console.error("Failed to trigger background job:", error);
      if (error.response?.status === 404) {
        return { error: "Job not found" };
      }
      if (error.response?.status === 409) {
        return { error: "Job is already running" };
      }
      return { error: error.response?.data?.error || "Failed to trigger job" };
    }
  };

  const fetchJobAuditLog = async (limit = 50, offset = 0) => {
    try {
      const response = await axios.get("/v1/admin/jobs/audit", {
        params: { limit, offset },
      });
      return response.data.entries;
    } catch (error) {
      console.error("Failed to fetch job audit log:", error);
      return null;
    }
  };

  const fetchJobAuditLogByJob = async (jobId, limit = 50, offset = 0) => {
    try {
      const response = await axios.get(`/v1/admin/jobs/${jobId}/audit`, {
        params: { limit, offset },
      });
      return response.data.entries;
    } catch (error) {
      console.error("Failed to fetch job audit log:", error);
      return null;
    }
  };

  const fetchRelevanceFilter = async () => {
    try {
      const response = await axios.get("/v1/admin/search/relevance-filter");
      return response.data;
    } catch (error) {
      console.error("Failed to fetch relevance filter:", error);
      return null;
    }
  };

  const updateRelevanceFilter = async (config) => {
    try {
      await axios.put("/v1/admin/search/relevance-filter", config);
      return { success: true };
    } catch (error) {
      console.error("Failed to update relevance filter:", error);
      return { error: error.response?.data?.error || "Failed to update filter" };
    }
  };

  // =====================================================
  // Admin API - Download Manager (DownloadManagerAdmin)
  // =====================================================

  const fetchDownloadStats = async () => {
    try {
      const response = await axios.get("/v1/download/admin/stats");
      return response.data;
    } catch (error) {
      console.error("Failed to fetch download stats:", error);
      return null;
    }
  };

  const fetchDownloadQueue = async () => {
    try {
      // Fetch non-completed requests (includes both parent items and standalone tracks)
      const response = await axios.get("/v1/download/admin/requests", {
        params: { limit: 200, offset: 0, exclude_completed: true, top_level_only: false },
      });
      return response.data;
    } catch (error) {
      console.error("Failed to fetch download queue:", error);
      return null;
    }
  };

  const fetchDownloadCompleted = async (limit = 100, offset = 0) => {
    try {
      // Fetch top-level completed requests
      const response = await axios.get("/v1/download/admin/requests", {
        params: { limit, offset, status: "COMPLETED", top_level_only: true },
      });
      return response.data;
    } catch (error) {
      console.error("Failed to fetch completed downloads:", error);
      return null;
    }
  };

  const fetchFailedDownloads = async (limit = 50, offset = 0) => {
    try {
      // Fetch top-level failed requests
      const response = await axios.get("/v1/download/admin/requests", {
        params: { limit, offset, status: "FAILED", top_level_only: true },
      });
      return response.data;
    } catch (error) {
      console.error("Failed to fetch failed downloads:", error);
      return null;
    }
  };

  const fetchDownloadActivity = async (limit = 50) => {
    try {
      const response = await axios.get("/v1/download/admin/activity", {
        params: { limit },
      });
      return response.data;
    } catch (error) {
      console.error("Failed to fetch download activity:", error);
      return null;
    }
  };

  /**
   * Fetch aggregated download statistics over time.
   * @param {string} period - "hourly" (48h), "daily" (30d), or "weekly" (12w). Default: daily
   * @param {number|null} since - Optional custom start time (unix timestamp)
   * @param {number|null} until - Optional custom end time (unix timestamp)
   * @returns {Object|null} Stats history with entries and totals, or null on error
   */
  const fetchDownloadStatsHistory = async (period = "daily", since = null, until = null) => {
    try {
      const params = { period };
      if (since !== null) params.since = since;
      if (until !== null) params.until = until;
      const response = await axios.get("/v1/download/admin/stats/history", {
        params,
      });
      return response.data;
    } catch (error) {
      console.error("Failed to fetch download stats history:", error);
      return null;
    }
  };

  const fetchDownloadRequests = async (limit = 100, offset = 0) => {
    try {
      const response = await axios.get("/v1/download/admin/requests", {
        params: { limit, offset },
      });
      return response.data;
    } catch (error) {
      console.error("Failed to fetch download requests:", error);
      return null;
    }
  };

  const fetchDownloadAuditLog = async (limit = 100, offset = 0) => {
    try {
      const response = await axios.get("/v1/download/admin/audit", {
        params: { limit, offset },
      });
      return response.data;
    } catch (error) {
      console.error("Failed to fetch download audit log:", error);
      return null;
    }
  };

  const fetchDownloadAuditForItem = async (itemId) => {
    try {
      const response = await axios.get(`/v1/download/admin/audit/item/${itemId}`);
      return response.data;
    } catch (error) {
      console.error("Failed to fetch audit for item:", error);
      return null;
    }
  };

  const fetchDownloadAuditForUser = async (userId) => {
    try {
      const response = await axios.get(`/v1/download/admin/audit/user/${userId}`);
      return response.data;
    } catch (error) {
      console.error("Failed to fetch audit for user:", error);
      return null;
    }
  };

  const retryDownload = async (itemId, force = false) => {
    try {
      const response = await axios.post(`/v1/download/admin/retry/${itemId}`, null, {
        params: { force },
      });
      return { success: true, data: response.data };
    } catch (error) {
      console.error("Failed to retry download:", error);
      if (error.response?.status === 400) {
        return { error: error.response.data || "Item not eligible for retry" };
      }
      if (error.response?.status === 404) {
        return { error: "Item not found" };
      }
      return { error: "Failed to retry download" };
    }
  };

  const deleteDownloadRequest = async (itemId) => {
    try {
      await axios.delete(`/v1/download/admin/request/${itemId}`);
      return { success: true };
    } catch (error) {
      console.error("Failed to delete download request:", error);
      if (error.response?.status === 400) {
        return { error: error.response.data || "Cannot delete item" };
      }
      if (error.response?.status === 404) {
        return { error: "Item not found" };
      }
      return { error: "Failed to delete download request" };
    }
  };

  const requestAlbumDownload = async (albumId, albumName, artistName) => {
    try {
      const response = await axios.post("/v1/download/request/album", {
        album_id: albumId,
        album_name: albumName,
        artist_name: artistName,
      });
      return { success: true, data: response.data };
    } catch (error) {
      console.error("Failed to request album download:", error);
      if (error.response?.data) {
        return { error: error.response.data };
      }
      return { error: "Failed to request download" };
    }
  };

  // =====================================================
  // Admin API - Changelog Batches (EditCatalog)
  // =====================================================

  const fetchChangelogBatches = async (isOpen = null) => {
    try {
      const params = {};
      if (isOpen !== null) params.is_open = isOpen;
      const response = await axios.get("/v1/admin/changelog/batches", { params });
      return response.data;
    } catch (error) {
      console.error("Failed to fetch changelog batches:", error);
      return null;
    }
  };

  const fetchChangelogBatch = async (batchId) => {
    try {
      const response = await axios.get(`/v1/admin/changelog/batch/${batchId}`);
      return response.data;
    } catch (error) {
      console.error("Failed to fetch changelog batch:", error);
      return null;
    }
  };

  const createChangelogBatch = async (name, description = null) => {
    try {
      const body = { name };
      if (description) body.description = description;
      const response = await axios.post("/v1/admin/changelog/batch", body);
      return { success: true, data: response.data };
    } catch (error) {
      console.error("Failed to create changelog batch:", error);
      if (error.response?.status === 409) {
        return { error: "A batch is already open. Close it first." };
      }
      return { error: "Failed to create batch" };
    }
  };

  const closeChangelogBatch = async (batchId) => {
    try {
      const response = await axios.post(`/v1/admin/changelog/batch/${batchId}/close`);
      return { success: true, data: response.data };
    } catch (error) {
      console.error("Failed to close changelog batch:", error);
      if (error.response?.status === 400) {
        return { error: "Batch is already closed" };
      }
      if (error.response?.status === 404) {
        return { error: "Batch not found" };
      }
      return { error: "Failed to close batch" };
    }
  };

  const deleteChangelogBatch = async (batchId) => {
    try {
      await axios.delete(`/v1/admin/changelog/batch/${batchId}`);
      return { success: true };
    } catch (error) {
      console.error("Failed to delete changelog batch:", error);
      if (error.response?.status === 400) {
        return { error: "Cannot delete batch with changes. Only empty batches can be deleted." };
      }
      if (error.response?.status === 404) {
        return { error: "Batch not found" };
      }
      return { error: "Failed to delete batch" };
    }
  };

  const fetchChangelogBatchChanges = async (batchId) => {
    try {
      const response = await axios.get(`/v1/admin/changelog/batch/${batchId}/changes`);
      return response.data;
    } catch (error) {
      console.error("Failed to fetch batch changes:", error);
      return null;
    }
  };

  // =====================================================
  // Admin API - Bug Reports (ServerAdmin)
  // =====================================================

  const fetchBugReports = async (limit = 50, offset = 0) => {
    try {
      const response = await axios.get("/v1/admin/bug-reports", {
        params: { limit, offset },
      });
      return response.data;
    } catch (error) {
      console.error("Failed to fetch bug reports:", error);
      return null;
    }
  };

  const getBugReport = async (id) => {
    try {
      const response = await axios.get(`/v1/admin/bug-report/${id}`);
      return response.data;
    } catch (error) {
      console.error("Failed to get bug report:", error);
      return null;
    }
  };

  const deleteBugReport = async (id) => {
    try {
      await axios.delete(`/v1/admin/bug-report/${id}`);
      return { success: true };
    } catch (error) {
      console.error("Failed to delete bug report:", error);
      if (error.response?.status === 404) {
        return { error: "Bug report not found" };
      }
      return { error: "Failed to delete bug report" };
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
    fetchUserSettings,
    updateUserSettings,
    fetchTrack,
    fetchResolvedTrack,
    fetchResolvedAlbum,
    fetchAlbum,
    fetchArtist,
    fetchArtistDiscography,
    fetchGenres,
    fetchGenreTracks,
    fetchGenreRadio,
    recordImpression,
    // Sync API
    fetchSyncState,
    fetchSyncEvents,
    // Admin API - User Management
    fetchAdminUsers,
    createUser,
    deleteUser,
    fetchUserRoles,
    addUserRole,
    removeUserRole,
    fetchUserPermissions,
    grantPermission,
    revokePermission,
    fetchUserCredentialsStatus,
    setUserPassword,
    deleteUserPassword,
    // Admin API - Analytics
    fetchDailyListening,
    fetchTopTracks,
    fetchTrackStats,
    fetchUserListeningSummary,
    fetchOnlineUsers,
    // Admin API - Server Control
    rebootServer,
    fetchBackgroundJobs,
    triggerBackgroundJob,
    fetchJobAuditLog,
    fetchJobAuditLogByJob,
    fetchRelevanceFilter,
    updateRelevanceFilter,
    // Admin API - Download Manager
    fetchDownloadStats,
    fetchDownloadQueue,
    fetchDownloadCompleted,
    fetchFailedDownloads,
    fetchDownloadActivity,
    fetchDownloadStatsHistory,
    fetchDownloadRequests,
    fetchDownloadAuditLog,
    fetchDownloadAuditForItem,
    fetchDownloadAuditForUser,
    retryDownload,
    deleteDownloadRequest,
    requestAlbumDownload,
    // Admin API - Changelog Batches
    fetchChangelogBatches,
    fetchChangelogBatch,
    createChangelogBatch,
    closeChangelogBatch,
    deleteChangelogBatch,
    fetchChangelogBatchChanges,
    // Admin API - Bug Reports
    fetchBugReports,
    getBugReport,
    deleteBugReport,
    // Admin API - Ingestion
    uploadIngestionFile,
    fetchIngestionMyJobs,
    fetchIngestionJob,
    processIngestionJob,
    convertIngestionJob,
    fetchIngestionReviews,
    resolveIngestionReview,
    fetchIngestionAdminJobs,
  };
});

// =====================================================
// Admin API - Ingestion (EditCatalog)
// =====================================================

async function uploadIngestionFile(filename, base64Data, contextType = null, contextId = null) {
  try {
    const body = {
      filename,
      data: base64Data,
    };
    if (contextType) body.context_type = contextType;
    if (contextId) body.context_id = contextId;
    const response = await axios.post("/v1/ingestion/upload", body);
    return { success: true, data: response.data };
  } catch (error) {
    console.error("Failed to upload file for ingestion:", error);
    return { error: error.response?.data?.error || "Failed to upload file" };
  }
}

async function fetchIngestionMyJobs(limit = 50) {
  try {
    const response = await axios.get("/v1/ingestion/my-jobs", {
      params: { limit },
    });
    return response.data;
  } catch (error) {
    console.error("Failed to fetch ingestion jobs:", error);
    return null;
  }
}

async function fetchIngestionJob(jobId) {
  try {
    const response = await axios.get(`/v1/ingestion/job/${jobId}`);
    return response.data;
  } catch (error) {
    console.error("Failed to fetch ingestion job:", error);
    return null;
  }
}

async function processIngestionJob(jobId) {
  try {
    const response = await axios.post(`/v1/ingestion/job/${jobId}/process`);
    return { success: true, data: response.data };
  } catch (error) {
    console.error("Failed to process ingestion job:", error);
    return { error: error.response?.data?.error || "Failed to process job" };
  }
}

async function convertIngestionJob(jobId) {
  try {
    const response = await axios.post(`/v1/ingestion/job/${jobId}/convert`);
    return { success: true, data: response.data };
  } catch (error) {
    console.error("Failed to convert ingestion job:", error);
    return { error: error.response?.data?.error || "Failed to convert job" };
  }
}

async function fetchIngestionReviews(limit = 50) {
  try {
    const response = await axios.get("/v1/ingestion/reviews", {
      params: { limit },
    });
    return response.data;
  } catch (error) {
    console.error("Failed to fetch ingestion reviews:", error);
    return null;
  }
}

async function resolveIngestionReview(jobId, selectedOption) {
  try {
    const response = await axios.post(`/v1/ingestion/review/${jobId}/resolve`, {
      selected_option: selectedOption,
    });
    return { success: true, data: response.data };
  } catch (error) {
    console.error("Failed to resolve ingestion review:", error);
    return { error: error.response?.data?.error || "Failed to resolve review" };
  }
}

async function fetchIngestionAdminJobs(limit = 50) {
  try {
    const response = await axios.get("/v1/ingestion/admin/jobs", {
      params: { limit },
    });
    return response.data;
  } catch (error) {
    console.error("Failed to fetch admin ingestion jobs:", error);
    return null;
  }
}
