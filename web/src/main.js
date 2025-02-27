import './assets/main.css'

import { createApp, watch } from 'vue'
import App from './App.vue'
import router from './router'
import { createPinia } from 'pinia'
import { useConfigStore } from './store/config'
import axios from 'axios';

window.addEventListener("contextmenu", (e) => { e.preventDefault() });

const pinia = createPinia();
const app = createApp(App);


app.use(pinia)
app.use(router)

window.config = useConfigStore();
app.mount('#app')

if (window.config.blockHttpCache) {
  axios.defaults.headers.common['Cache-Control'] = 'no-cache, no-store, must-revalidate';
  axios.defaults.headers.common['Pragma'] = 'no-cache';
  axios.defaults.headers.common['Expires'] = '0';
}

watch(() => window.config.blockHttpCache, (value) => {
  console.log("blockHttpCache changed, reloading page");
  window.location.reload();
});

