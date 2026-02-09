import "./assets/main.css";

import { createApp, watch } from "vue";
import App from "./App.vue";
import router from "./router";
import { createPinia } from "pinia";
import { useDebugStore } from "./store/debug";
import { useRemoteStore } from "./store/remote";
import { useAuthStore } from "./store/auth";
import { setupAxiosInterceptors } from "./services/api";

// Setup axios interceptors for auth token handling BEFORE creating stores
setupAxiosInterceptors();

const pinia = createPinia();
const app = createApp(App);

app.use(pinia);
app.use(router);

window.config = useDebugStore();
const remoteStore = useRemoteStore();
const authStore = useAuthStore();

// Initialize auth store (checks for existing OIDC session)
// This is async but we don't need to wait - the router guard will handle it
authStore.initialize();

app.mount("#app");

if ("serviceWorker" in navigator) {
  window.addEventListener("load", async () => {
    try {
      await navigator.serviceWorker.register("/sw.js");
      console.log("[SW] Registered");
      if (!navigator.serviceWorker.controller) {
        const reloadKey = "sw_controller_reloaded";
        navigator.serviceWorker.addEventListener("controllerchange", () => {
          if (sessionStorage.getItem(reloadKey)) return;
          sessionStorage.setItem(reloadKey, "1");
          window.location.reload();
        });
      }
    } catch (error) {
      console.warn("[SW] Registration failed:", error);
    }
  });
}

remoteStore.setBlockHttpCache(window.config.blockHttpCache);

watch(
  () => window.config.blockHttpCache,
  () => {
    console.log("blockHttpCache changed, reloading page");
    window.location.reload();
  },
);

const rightClickBlocker = (e) => {
  e.preventDefault();
};

watch(
  () => window.config.blockRightClick,
  (value) => {
    if (value) {
      window.addEventListener("contextmenu", rightClickBlocker);
    } else {
      window.removeEventListener("contextmenu", rightClickBlocker);
    }
  },
);
