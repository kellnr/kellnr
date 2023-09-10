import { createApp } from 'vue';
import App from './App.vue';
import router from './router';
// @ts-ignore
import VueHighlightJS from 'vue3-highlightjs'; // https://www.npmjs.com/package/vue3-highlightjs
import axios from 'axios';
import VueAxios from 'vue-axios';
import { store } from "@/store/store";
import 'highlight.js/styles/default.css';
import './assets/css/main.css';
import '../node_modules/bulma/css/bulma.min.css';
import '../node_modules/bulma-switch/dist/css/bulma-switch.min.css';
import '../node_modules/@fortawesome/fontawesome-free/js/all';
createApp(App)
    .use(router)
    .use(store)
    .use(VueHighlightJS)
    .use(VueAxios, axios)
    .mount('#app');
//# sourceMappingURL=main.js.map