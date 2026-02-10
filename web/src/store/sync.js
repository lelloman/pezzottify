import { defineStore } from "pinia";
import { ref } from "vue";
import { useRemoteStore } from "./remote";
import { useUserStore } from "./user";
import { useStaticsStore } from "./statics";

// localStorage key for sync cursor
const SYNC_CURSOR_KEY = "pezzottify_sync_cursor";

export const useSyncStore = defineStore("sync", () => {
  const remoteStore = useRemoteStore();
  const userStore = useUserStore();
  const staticsStore = useStaticsStore();

  const cursor = ref(0);
  const isInitialized = ref(false);
  const isInitializing = ref(false);
  const isSyncing = ref(false);
  const lastSyncError = ref(null);

  // =====================================================
  // Cursor Persistence
  // =====================================================

  const loadCursor = () => {
    try {
      const stored = localStorage.getItem(SYNC_CURSOR_KEY);
      if (stored) {
        cursor.value = parseInt(stored, 10) || 0;
      }
    } catch (error) {
      console.error("Failed to load sync cursor:", error);
      cursor.value = 0;
    }
  };

  const saveCursor = (newCursor) => {
    try {
      cursor.value = newCursor;
      localStorage.setItem(SYNC_CURSOR_KEY, String(newCursor));
    } catch (error) {
      console.error("Failed to save sync cursor:", error);
    }
  };

  const clearCursor = () => {
    try {
      cursor.value = 0;
      localStorage.removeItem(SYNC_CURSOR_KEY);
    } catch (error) {
      console.error("Failed to clear sync cursor:", error);
    }
  };

  // =====================================================
  // Event Application
  // =====================================================

  const applyEvent = (event) => {
    // Event structure from server:
    // { seq: number, type: string, payload: object, server_timestamp: number }
    const { type, payload } = event;

    switch (type) {
      case "content_liked":
        userStore.applyContentLiked(payload.content_type, payload.content_id);
        break;

      case "content_unliked":
        userStore.applyContentUnliked(payload.content_type, payload.content_id);
        break;

      case "setting_changed":
        userStore.applySettingChanged(payload.setting);
        break;

      case "playlist_created":
        userStore.applyPlaylistCreated(payload.playlist_id, payload.name);
        break;

      case "playlist_renamed":
        userStore.applyPlaylistRenamed(payload.playlist_id, payload.name);
        break;

      case "playlist_deleted":
        userStore.applyPlaylistDeleted(payload.playlist_id);
        break;

      case "playlist_tracks_updated":
        userStore.applyPlaylistTracksUpdated(
          payload.playlist_id,
          payload.track_ids,
        );
        break;

      case "permission_granted":
        userStore.applyPermissionGranted(payload.permission);
        break;

      case "permission_revoked":
        userStore.applyPermissionRevoked(payload.permission);
        break;

      case "permissions_reset":
        userStore.applyPermissionsReset(payload.permissions);
        break;

      case "notification_created":
        userStore.applyNotificationCreated(payload.notification);
        break;

      case "notification_read":
        userStore.applyNotificationRead(payload.notification_id, payload.read_at);
        break;

      case "catalog_invalidation": {
        // Invalidate cached content when catalog changes
        const contentType = payload.content_type; // "album", "artist", "track"
        const contentId = payload.content_id;
        const typeMap = { album: "albums", artist: "artists", track: "tracks" };
        const itemType = typeMap[contentType];
        if (itemType) {
          staticsStore.invalidateItem(itemType, contentId);
        }
        break;
      }

      default:
        console.warn("Unknown sync event type:", type);
    }
  };

  // =====================================================
  // Full Sync
  // =====================================================

  const fullSync = async () => {
    console.log("Performing full sync...");
    isSyncing.value = true;
    lastSyncError.value = null;

    try {
      const state = await remoteStore.fetchSyncState();

      // Update all user state from sync response
      userStore.setLikedAlbums(state.likes?.albums || []);
      userStore.setLikedArtists(state.likes?.artists || []);
      userStore.setLikedTracks(state.likes?.tracks || []);

      // Convert settings array to object format
      const settingsObj = {};
      if (state.settings) {
        state.settings.forEach((setting) => {
          // Handle different setting formats from server
          if (typeof setting === "object") {
            // Format 1: Tagged format { key: "enable_...", value: true }
            if ("key" in setting && "value" in setting) {
              const value =
                typeof setting.value === "boolean"
                  ? setting.value
                    ? "true"
                    : "false"
                  : String(setting.value);
              settingsObj[setting.key] = value;
            } else {
              // Handle direct key-value pairs
              const key = Object.keys(setting)[0];
              settingsObj[key] = setting[key];
            }
          }
        });
      }
      userStore.setAllSettings(settingsObj);

      // Convert playlists to expected format
      const playlists = (state.playlists || []).map((p) => ({
        id: p.id,
        name: p.name,
        tracks: p.tracks || [],
      }));
      userStore.setPlaylists(playlists);

      userStore.setPermissions(state.permissions || []);
      userStore.setNotifications(state.notifications || []);

      // Update cursor
      saveCursor(state.seq);

      console.log("Full sync complete, cursor:", state.seq);
      return true;
    } catch (error) {
      console.error("Full sync failed:", error);
      lastSyncError.value = error;
      return false;
    } finally {
      isSyncing.value = false;
    }
  };

  // =====================================================
  // Catch-Up Sync
  // =====================================================

  const catchUp = async () => {
    console.log("Catching up from cursor:", cursor.value);
    isSyncing.value = true;
    lastSyncError.value = null;

    try {
      const result = await remoteStore.fetchSyncEvents(cursor.value);

      // Check for 410 Gone (events pruned)
      if (result.error === "events_pruned") {
        console.log("Events pruned, performing full sync");
        return await fullSync();
      }

      const { events, current_seq } = result;

      // Check for sequence gap
      if (events.length > 0 && events[0].seq > cursor.value + 1) {
        console.log("Sequence gap detected, performing full sync");
        return await fullSync();
      }

      // Apply events in order
      for (const event of events) {
        applyEvent(event);
        saveCursor(event.seq);
      }

      // Update cursor to current even if no events
      if (current_seq > cursor.value) {
        saveCursor(current_seq);
      }

      console.log("Catch-up complete, cursor:", cursor.value);
      return true;
    } catch (error) {
      console.error("Catch-up failed:", error);
      lastSyncError.value = error;
      return false;
    } finally {
      isSyncing.value = false;
    }
  };

  // =====================================================
  // WebSocket Event Handler
  // =====================================================

  const handleSyncMessage = (type, payload) => {
    // Handler receives (type, payload) from WebSocket service
    // payload format: { event: StoredEvent }
    console.log("[Sync] Received message:", type, payload);
    const event = payload?.event;
    if (!event) {
      console.warn("[Sync] Invalid sync message - no event:", type, payload);
      return;
    }

    // Check for sequence gap
    if (event.seq > cursor.value + 1) {
      console.log("WebSocket sequence gap detected, catching up");
      catchUp();
      return;
    }

    // Apply the event
    applyEvent(event);
    saveCursor(event.seq);
  };

  // =====================================================
  // Initialization and Cleanup
  // =====================================================

  const initialize = async () => {
    if (isInitialized.value) return true;
    if (isInitializing.value) return false;

    isInitializing.value = true;

    try {
      // Load cursor from storage
      loadCursor();

      // Check if user store has in-memory state
      // On page reload, the cursor is preserved but in-memory state is lost
      const hasInMemoryState = userStore.likedTrackIds !== null;
      const needsFullSync = cursor.value === 0 || !hasInMemoryState;

      if (needsFullSync) {
        await fullSync();
      } else {
        await catchUp();
      }

      isInitialized.value = true;
      return true;
    } catch (error) {
      console.error("Sync initialization failed:", error);
      lastSyncError.value = error;
      return false;
    } finally {
      isInitializing.value = false;
    }
  };

  const cleanup = () => {
    clearCursor();
    isInitialized.value = false;
    lastSyncError.value = null;
  };

  return {
    // State
    cursor,
    isInitialized,
    isInitializing,
    isSyncing,
    lastSyncError,

    // Methods
    initialize,
    cleanup,
    fullSync,
    catchUp,
    handleSyncMessage,
    applyEvent,
  };
});

// Register catalog invalidation handler with WebSocket
// This must be at module level to ensure registration on import
import { registerHandler } from "@/services/websocket";

registerHandler("catalog", (type, payload) => {
  if (type === "catalog_invalidation") {
    const syncStore = useSyncStore();
    syncStore.applyEvent({
      type: "catalog_invalidation",
      payload: payload,
    });
  }
});
