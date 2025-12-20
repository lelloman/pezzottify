import { defineStore } from "pinia";
import axios from "axios";
import * as ws from "../services/websocket";
import { useSyncStore } from "./sync";

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
     * Redirects the browser to the OIDC login endpoint.
     */
    loginWithOidc() {
      window.location.href = "/v1/auth/oidc/login";
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
