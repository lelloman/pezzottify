import { defineStore } from "pinia";
import axios from "axios";
import * as ws from "../services/websocket";
import * as oidc from "../services/oidc";
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
     * Redirects the browser to the OIDC provider.
     */
    async loginWithOidc() {
      await oidc.login();
    },

    /**
     * Handle OIDC callback after redirect from provider.
     * Called from the callback route.
     */
    async handleOidcCallback() {
      try {
        const user = await oidc.handleCallback();
        if (user) {
          // Verify the session with the backend
          return await this.checkSession();
        }
        return false;
      } catch (error) {
        console.error("OIDC callback failed:", error);
        return false;
      }
    },

    /**
     * Check if the user has a valid session.
     * Uses the ID token from OIDC to authenticate with the backend.
     */
    async checkSession() {
      try {
        // First check if we have OIDC tokens
        const idToken = await oidc.getIdToken();
        if (!idToken) {
          this.user = null;
          this.sessionChecked = true;
          return false;
        }

        // Verify with backend using the ID token
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
      } catch (error) {
        // 401/403 means no valid session
        console.debug("Session check failed:", error?.response?.status);
        this.user = null;
        this.sessionChecked = true;
        return false;
      }
    },

    /**
     * Attempt to refresh tokens.
     * Returns true if refresh was successful.
     */
    async refreshTokens() {
      try {
        const newUser = await oidc.refreshTokens();
        if (newUser) {
          // Verify the new session with the backend
          return await this.checkSession();
        }
        return false;
      } catch (error) {
        console.error("Token refresh failed:", error);
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

      // Clear OIDC tokens
      await oidc.logout(false);

      this.user = null;
      this.sessionChecked = false;
    },

    /**
     * Initialize the auth store on app startup.
     * Checks for existing OIDC session.
     */
    async initialize() {
      await this.checkSession();
    },
  },
});
