import "./assets/main.css";

import { createApp, watch } from "vue";
import App from "./App.vue";
import router from "./router";
import { createPinia } from "pinia";
import { useDebugStore } from "./store/debug";
import { useRemoteStore } from "./store/remote";
import { useAuthStore } from "./store/auth";

const pinia = createPinia();
const app = createApp(App);

app.use(pinia);
app.use(router);

window.config = useDebugStore();
const remoteStore = useRemoteStore();
const authStore = useAuthStore();

// Initialize auth store (connects to WebSocket if already authenticated)
authStore.initialize();

app.mount("#app");

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
