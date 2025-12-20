import { defineStore } from "pinia";
import axios from "axios";
import * as ws from "../services/websocket";
import { useSyncStore } from "./sync";

const DEVICE_ID_KEY = "pezzottify_device_id";

/**
 * Get or generate a persistent device ID.
 * Stored in localStorage to persist across sessions.
 */
function getDeviceId() {
  let deviceId = localStorage.getItem(DEVICE_ID_KEY);
  if (!deviceId) {
    deviceId = crypto.randomUUID();
    localStorage.setItem(DEVICE_ID_KEY, deviceId);
  }
  return deviceId;
}

/**
 * Get a human-readable device name based on browser/platform info.
 */
function getDeviceName() {
  const userAgent = navigator.userAgent;
  let browser = "Browser";
  let os = "Unknown";

  // Detect browser (order matters: Edge UA contains "Chrome", Chrome UA contains "Safari")
  if (userAgent.includes("Edg/")) browser = "Edge";
  else if (userAgent.includes("Firefox")) browser = "Firefox";
  else if (userAgent.includes("Chrome")) browser = "Chrome";
  else if (userAgent.includes("Safari")) browser = "Safari";

  // Detect OS
  if (userAgent.includes("Windows")) os = "Windows";
  else if (userAgent.includes("Mac")) os = "macOS";
  else if (userAgent.includes("Linux")) os = "Linux";
  else if (userAgent.includes("Android")) os = "Android";
  else if (userAgent.includes("iPhone") || userAgent.includes("iPad"))
    os = "iOS";

  return `${browser} on ${os}`;
}

export const useAuthStore = defineStore("auth", {
  state: () => ({
    user: null,
    sessionChecked: false,
  }),
  getters: {
    isAuthenticated: (state) => !!state.user,
  },
  actions: {
    /**
     * Initiate OIDC login flow.
     * Redirects the browser to the OIDC login endpoint with device info.
     */
    loginWithOidc() {
      const deviceId = getDeviceId();
      const deviceType = "web";
      const deviceName = getDeviceName();

      const params = new URLSearchParams({
        device_id: deviceId,
        device_type: deviceType,
        device_name: deviceName,
      });

      window.location.href = `/v1/auth/oidc/login?${params.toString()}`;
    },

    /**
     * Check if the user has a valid session.
     * Called on app startup and after OIDC callback redirect.
     */
    async checkSession() {
      try {
        const response = await axios.get("/v1/auth/session");
        this.user = {
          handle: response.data.user_handle,
          permissions: response.data.permissions,
        };
        this.sessionChecked = true;

        // Connect to WebSocket after confirming session
        const syncStore = useSyncStore();
        ws.registerHandler("sync", syncStore.handleSyncMessage);
        ws.connect();

        return true;
      } catch {
        // 401/403 means no valid session
        this.user = null;
        this.sessionChecked = true;
        return false;
      }
    },

    async logout() {
      // Unregister sync handler and disconnect WebSocket before clearing auth
      ws.unregisterHandler("sync");
      ws.disconnect();

      // Cleanup sync state and reset user store
      try {
        const { useSyncStore } = await import("./sync");
        const { useUserStore } = await import("./user");
        const syncStore = useSyncStore();
        const userStore = useUserStore();

        syncStore.cleanup();
        userStore.reset();
      } catch (error) {
        console.error("Failed to cleanup stores:", error);
      }

      this.user = null;
      this.sessionChecked = false;
    },

    /**
     * Initialize the auth store on app startup.
     * Checks for existing session via cookie.
     */
    async initialize() {
      await this.checkSession();
    },
  },
});
