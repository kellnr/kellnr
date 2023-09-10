import { createApp } from 'vue'
import App from './App.vue'
// @ts-ignore
import VueHighlightJS from 'vue3-highlightjs' // https://www.npmjs.com/package/vue3-highlightjs
import Axios from 'axios'
import { setupCache } from 'axios-cache-interceptor';
import VueAxios from 'vue-axios'
import {store} from "@/store/store";
import router from "@/router";

import 'highlight.js/styles/default.css'
import './assets/css/main.css'
import '../node_modules/bulma/css/bulma.min.css'
import '../node_modules/bulma-switch/dist/css/bulma-switch.min.css'
import '../node_modules/@fortawesome/fontawesome-free/js/all'

const axios = setupCache(Axios);


createApp(App)
    .use(router)
    .use(store)
    .use(VueHighlightJS)
    // @ts-ignore
    .use(VueAxios, axios)
    .mount('#app')
