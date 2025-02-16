import './assets/main.css'

import { createApp } from 'vue'
import App from './App.vue'
import router from './router'
import { createPinia } from 'pinia'
import { useConfigStore } from './store/config'


const pinia = createPinia();
const app = createApp(App);


app.use(pinia)
app.use(router)

window.config = useConfigStore();
app.mount('#app')

