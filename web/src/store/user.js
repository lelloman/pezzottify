import { defineStore } from "pinia";
import { ref, computed } from "vue";
import { useRemoteStore } from "./remote";

// Settings key constants
export const SETTING_ENABLE_EXTERNAL_SEARCH = "enable_external_search";

// Admin permissions that grant access to admin panel
const ADMIN_PERMISSIONS = [
  "ManagePermissions",
  "ViewAnalytics",
  "ServerAdmin",
];

export const useUserStore = defineStore("user", () => {
  const remoteStore = useRemoteStore();
  const likedAlbumIds = ref(null);
  const likedArtistsIds = ref(null);
  const likedTrackIds = ref(null);
  const playlistsData = ref(null);
  const playlistRefs = {};
  const settings = ref({});
  const permissions = ref([]);

  // Pending settings that failed to sync - key -> { value, retryCount }
  const pendingSettings = ref({});
  const MAX_RETRY_COUNT = 3;
  const RETRY_DELAY_MS = 2000;

  const isInitialized = ref(false);
  const isInitializing = ref(false);

  // Load all user data via sync
  const initialize = async () => {
    // Return early if already initialized and not forcing refresh
    if (isInitialized.value) return true;

    // Return early if already initializing
    if (isInitializing.value) return false;

    isInitializing.value = true;

    try {
      // Use lazy import to avoid circular dependency
      const { useSyncStore } = await import("./sync");
      const syncStore = useSyncStore();

      // Initialize via sync store - this will do full sync or catch-up
      const success = await syncStore.initialize();

      if (success) {
        isInitialized.value = true;
      }
      return success;
    } catch (error) {
      console.error("Failed to initialize user data:", error);
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
        likedAlbumIds.value = likedAlbumIds.value.filter(
          (id) => id !== albumId,
        );
      }
    }
  };

  const setArtistIsLiked = async (artistId, isLiked) => {
    const success = await remoteStore.setArtistLikeStatus(artistId, isLiked);
    if (success) {
      if (isLiked) {
        likedArtistsIds.value = [artistId, ...likedArtistsIds.value];
      } else {
        likedArtistsIds.value = likedArtistsIds.value.filter(
          (id) => id !== artistId,
        );
      }
    }
  };

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
      playlistsData.value.list = playlistsData.value.list.map((playlist) => {
        if (playlist.id === playlistId) {
          return playlistData;
        }
        return playlist;
      });
      playlistsData.value.by_id[playlistId] = playlistData;
    }
  };

  const createPlaylist = async (callback) => {
    const newPlaylist = await remoteStore.createNewPlaylist();
    if (newPlaylist && playlistsData.value) {
      console.log("Creating new playlist");
      console.log(newPlaylist);
      playlistsData.value.list = [newPlaylist, ...playlistsData.value.list];
      playlistsData.value.by_id[newPlaylist.id] = newPlaylist;
    }
    callback(newPlaylist);
  };

  const deletePlaylist = async (playlistId, callback) => {
    const success = await remoteStore.deleteUserPlaylist(playlistId);
    if (success && playlistsData.value) {
      const oldValue = playlistsData.value;
      delete oldValue.by_id[playlistId];
      oldValue.list = oldValue.list.filter(
        (playlist) => playlist !== playlistId,
      );
      playlistsData.value = oldValue;
      if (playlistRefs[playlistId]) {
        delete playlistRefs[playlistId];
      }
    }
    callback(success);
  };

  const updatePlaylistName = async (playlistId, name, callback) => {
    const success = await remoteStore.updatePlaylistName(playlistId, name);
    if (
      success &&
      playlistsData.value &&
      playlistsData.value.by_id[playlistId]
    ) {
      // Update name in memory
      playlistsData.value.by_id[playlistId].name = name;

      // Update the playlist in the list
      playlistsData.value.list = playlistsData.value.list.map((p) =>
        p.id === playlistId ? { ...p, name } : p,
      );
    }
    callback(success);
  };

  const addTracksToPlaylist = async (playlistId, trackIds, callback) => {
    const success = await remoteStore.addTracksToPlaylist(playlistId, trackIds);
    console.log("user store addTracksToPlaylist success: " + success);
    if (
      success &&
      playlistsData.value &&
      playlistsData.value.by_id[playlistId]
    ) {
      const playlist = playlistsData.value.by_id[playlistId];
      playlist.tracks = [...playlist.tracks, ...trackIds];
      console.log("user store addTracksToPlaylist playlist:");
    }
    callback(success);
  };

  const removeTracksFromPlaylist = async (
    playlistId,
    tracksPositions,
    callback,
  ) => {
    const success = await remoteStore.removeTracksFromPlaylist(
      playlistId,
      tracksPositions,
    );
    if (
      success &&
      playlistsData.value &&
      playlistsData.value.by_id[playlistId]
    ) {
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
  };

  // Settings methods
  const getSetting = (key) => {
    return settings.value[key];
  };

  // Check if a setting has a pending sync
  const isSettingPending = (key) => {
    return key in pendingSettings.value;
  };

  // Check if any settings are pending sync
  const hasPendingSettings = computed(() => {
    return Object.keys(pendingSettings.value).length > 0;
  });

  // Internal function to sync a single setting to the server
  const syncSettingToServer = async (key, value, retryCount = 0) => {
    const success = await remoteStore.updateUserSettings({ [key]: value });

    if (success) {
      // Remove from pending on success
      const newPending = { ...pendingSettings.value };
      delete newPending[key];
      pendingSettings.value = newPending;
      return true;
    } else {
      // Track as pending for retry
      pendingSettings.value = {
        ...pendingSettings.value,
        [key]: { value, retryCount },
      };

      // Schedule retry if under max count
      if (retryCount < MAX_RETRY_COUNT) {
        setTimeout(
          () => {
            // Only retry if still pending with same value
            const pending = pendingSettings.value[key];
            if (pending && pending.value === value) {
              syncSettingToServer(key, value, retryCount + 1);
            }
          },
          RETRY_DELAY_MS * Math.pow(2, retryCount),
        ); // Exponential backoff
      }
      return false;
    }
  };

  // Optimistic setting update - updates UI immediately, syncs in background
  const setSetting = async (key, value) => {
    // Optimistically update local state immediately
    settings.value = { ...settings.value, [key]: value };

    // Sync to server in background (don't await)
    syncSettingToServer(key, value);

    // Always return true since we updated optimistically
    return true;
  };

  // Retry all pending settings (call on reconnect or user action)
  const retryPendingSettings = async () => {
    const pending = { ...pendingSettings.value };
    for (const [key, { value }] of Object.entries(pending)) {
      await syncSettingToServer(key, value, 0);
    }
  };

  // Convenience computed for external search setting
  const isExternalSearchEnabled = computed(() => {
    return settings.value[SETTING_ENABLE_EXTERNAL_SEARCH] === "true";
  });

  const isExternalSearchPending = computed(() => {
    return isSettingPending(SETTING_ENABLE_EXTERNAL_SEARCH);
  });

  const setExternalSearchEnabled = async (enabled) => {
    return await setSetting(
      SETTING_ENABLE_EXTERNAL_SEARCH,
      enabled ? "true" : "false",
    );
  };

  // =====================================================
  // Sync Event Apply Methods
  // These methods apply incoming sync events to local state
  // =====================================================

  const applyContentLiked = (contentType, contentId) => {
    switch (contentType) {
      case "album":
        if (likedAlbumIds.value && !likedAlbumIds.value.includes(contentId)) {
          likedAlbumIds.value = [contentId, ...likedAlbumIds.value];
        }
        break;
      case "artist":
        if (
          likedArtistsIds.value &&
          !likedArtistsIds.value.includes(contentId)
        ) {
          likedArtistsIds.value = [contentId, ...likedArtistsIds.value];
        }
        break;
      case "track":
        if (likedTrackIds.value && !likedTrackIds.value.includes(contentId)) {
          likedTrackIds.value = [contentId, ...likedTrackIds.value];
        }
        break;
    }
  };

  const applyContentUnliked = (contentType, contentId) => {
    switch (contentType) {
      case "album":
        if (likedAlbumIds.value) {
          likedAlbumIds.value = likedAlbumIds.value.filter(
            (id) => id !== contentId,
          );
        }
        break;
      case "artist":
        if (likedArtistsIds.value) {
          likedArtistsIds.value = likedArtistsIds.value.filter(
            (id) => id !== contentId,
          );
        }
        break;
      case "track":
        if (likedTrackIds.value) {
          likedTrackIds.value = likedTrackIds.value.filter(
            (id) => id !== contentId,
          );
        }
        break;
    }
  };

  const applySettingChanged = (setting) => {
    // Setting comes from sync events in tagged format: { key: "setting_key", value: settingValue }
    if (typeof setting === "object") {
      // Handle tagged format from sync events: { key: "...", value: ... }
      if ("key" in setting && "value" in setting) {
        const value =
          typeof setting.value === "boolean"
            ? setting.value
              ? "true"
              : "false"
            : setting.value;
        settings.value = { ...settings.value, [setting.key]: value };
      }
      // Handle legacy format: { ExternalSearchEnabled: true }
      else if ("ExternalSearchEnabled" in setting) {
        settings.value = {
          ...settings.value,
          [SETTING_ENABLE_EXTERNAL_SEARCH]: setting.ExternalSearchEnabled
            ? "true"
            : "false",
        };
      } else {
        // Handle direct key-value pairs
        settings.value = { ...settings.value, ...setting };
      }
    }
  };

  const applyPlaylistCreated = (playlistId, name) => {
    if (playlistsData.value) {
      const newPlaylist = {
        id: playlistId,
        name: name,
        tracks: [],
      };
      // Only add if not already present
      if (!playlistsData.value.by_id[playlistId]) {
        playlistsData.value.list = [newPlaylist, ...playlistsData.value.list];
        playlistsData.value.by_id[playlistId] = newPlaylist;
      }
    }
  };

  const applyPlaylistRenamed = (playlistId, name) => {
    if (playlistsData.value && playlistsData.value.by_id[playlistId]) {
      playlistsData.value.by_id[playlistId].name = name;
      playlistsData.value.list = playlistsData.value.list.map((p) =>
        p.id === playlistId ? { ...p, name } : p,
      );
    }
  };

  const applyPlaylistDeleted = (playlistId) => {
    if (playlistsData.value) {
      delete playlistsData.value.by_id[playlistId];
      playlistsData.value.list = playlistsData.value.list.filter(
        (p) => p.id !== playlistId,
      );
      if (playlistRefs[playlistId]) {
        delete playlistRefs[playlistId];
      }
    }
  };

  const applyPlaylistTracksUpdated = (playlistId, trackIds) => {
    if (playlistsData.value && playlistsData.value.by_id[playlistId]) {
      playlistsData.value.by_id[playlistId].tracks = trackIds;
    }
  };

  const applyPermissionGranted = (permission) => {
    if (!permissions.value.includes(permission)) {
      permissions.value = [...permissions.value, permission];
    }
  };

  const applyPermissionRevoked = (permission) => {
    permissions.value = permissions.value.filter((p) => p !== permission);
  };

  const applyPermissionsReset = (newPermissions) => {
    permissions.value = newPermissions;
  };

  // =====================================================
  // Permission Check Helpers
  // =====================================================

  const hasPermission = (permission) => {
    return permissions.value.includes(permission);
  };

  const hasAnyAdminPermission = computed(() => {
    return ADMIN_PERMISSIONS.some((p) => permissions.value.includes(p));
  });

  const canManagePermissions = computed(() =>
    permissions.value.includes("ManagePermissions"),
  );
  const canViewAnalytics = computed(() =>
    permissions.value.includes("ViewAnalytics"),
  );
  const canServerAdmin = computed(() =>
    permissions.value.includes("ServerAdmin"),
  );
  const canRequestContent = computed(() =>
    permissions.value.includes("RequestContent"),
  );

  // =====================================================
  // Setter Methods for Full Sync
  // These methods set the full state from sync API response
  // =====================================================

  const setLikedAlbums = (albumIds) => {
    likedAlbumIds.value = albumIds;
  };

  const setLikedArtists = (artistIds) => {
    likedArtistsIds.value = artistIds;
  };

  const setLikedTracks = (trackIds) => {
    likedTrackIds.value = trackIds;
  };

  const setAllSettings = (newSettings) => {
    settings.value = newSettings;
  };

  const setPlaylists = (playlists) => {
    const by_id = {};
    playlists.forEach((playlist) => {
      by_id[playlist.id] = playlist;
    });
    playlistsData.value = {
      list: playlists,
      by_id: by_id,
    };
  };

  const setPermissions = (newPermissions) => {
    permissions.value = newPermissions;
  };

  // Reset all state (for logout)
  const reset = () => {
    likedAlbumIds.value = null;
    likedArtistsIds.value = null;
    likedTrackIds.value = null;
    playlistsData.value = null;
    settings.value = {};
    pendingSettings.value = {};
    permissions.value = [];
    isInitialized.value = false;
    isInitializing.value = false;
    // Clear playlist refs
    Object.keys(playlistRefs).forEach((key) => delete playlistRefs[key]);
  };

  return {
    // State
    likedAlbumIds,
    likedArtistsIds,
    likedTrackIds,
    playlistsData,
    settings,
    permissions,
    isInitialized,
    isInitializing,

    // Standard methods
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
    isSettingPending,
    hasPendingSettings,
    retryPendingSettings,
    isExternalSearchEnabled,
    isExternalSearchPending,
    setExternalSearchEnabled,

    // Sync event apply methods
    applyContentLiked,
    applyContentUnliked,
    applySettingChanged,
    applyPlaylistCreated,
    applyPlaylistRenamed,
    applyPlaylistDeleted,
    applyPlaylistTracksUpdated,
    applyPermissionGranted,
    applyPermissionRevoked,
    applyPermissionsReset,

    // Full sync setter methods
    setLikedAlbums,
    setLikedArtists,
    setLikedTracks,
    setAllSettings,
    setPlaylists,
    setPermissions,

    // Lifecycle
    reset,

    // Permission check helpers
    hasPermission,
    hasAnyAdminPermission,
    canManagePermissions,
    canViewAnalytics,
    canServerAdmin,
    canRequestContent,
  };
});
