import { createApp } from 'vue'
import App from './App.vue'
import VueHighlightJS from 'vue3-highlightjs' // https://www.npmjs.com/package/vue3-highlightjs
import Axios from 'axios'
import { setupCache } from 'axios-cache-interceptor';
import VueAxios from 'vue-axios'
import { createPinia } from 'pinia'
import piniaPluginPersistedstate from 'pinia-plugin-persistedstate'
import router from "./router";

import 'highlight.js/styles/default.css'
import './assets/css/main.css'
import '../node_modules/bulma/css/bulma.min.css'
import '../node_modules/bulma-switch/dist/css/bulma-switch.min.css'
import '../node_modules/@fortawesome/fontawesome-free/js/all'

const axios = setupCache(Axios);
const pinia = createPinia()
pinia.use(piniaPluginPersistedstate)

createApp(App)
    .use(pinia)
    .use(router)
    .use(VueHighlightJS)
    // @ts-expect-error TS doesn't understand axios cache
    .use(VueAxios, axios)
    .mount('#app')
