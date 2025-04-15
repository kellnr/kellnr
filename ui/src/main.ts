import { createApp } from 'vue'
import App from './App.vue'
import VueHighlightJS from 'vue3-highlightjs' // https://www.npmjs.com/package/vue3-highlightjs
import Axios from 'axios'
import { setupCache } from 'axios-cache-interceptor';
import VueAxios from 'vue-axios'
import { createPinia } from 'pinia'
import piniaPluginPersistedstate from 'pinia-plugin-persistedstate'
import router from "./router";

// Vuetify
import 'vuetify/styles'
import { createVuetify } from 'vuetify'
import * as components from 'vuetify/components'
import * as directives from 'vuetify/directives'
// Material Design Icons
import '@mdi/font/css/materialdesignicons.css'

import 'highlight.js/styles/default.css'
import './assets/css/main.css'
import '../node_modules/@fortawesome/fontawesome-free/js/all'

const axios = setupCache(Axios);
const pinia = createPinia()
pinia.use(piniaPluginPersistedstate)

// Create Vuetify instance
const vuetify = createVuetify({
    components,
    directives,
    theme: {
        defaultTheme: 'light',
        themes: {
            light: {
                colors: {
                    primary: '#1867C0',
                    secondary: '#5CBBF6',
                    background: '#FFFFFF',
                    surface: '#FFFFFF',
                }
            },
            dark: {
                colors: {
                    primary: '#2196F3',
                    secondary: '#424242',
                    background: '#121212',
                    surface: '#212121',
                }
            }
        }
    },
    icons: {
        defaultSet: 'mdi' // Use Material Design Icons as default
    }
})

createApp(App)
    .use(pinia)
    .use(router)
    .use(VueHighlightJS)
    // @ts-expect-error TS doesn't understand axios cache
    .use(VueAxios, axios)
    .use(vuetify) // Add Vuetify to your app
    .mount('#app')

