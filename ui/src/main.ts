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

// Import both light and dark highlight.js themes
import 'highlight.js/styles/github.css' // Light theme
import "highlight.js/styles/vs2015.css"; // Good dark theme alternative

const axios = setupCache(Axios);
const pinia = createPinia()
pinia.use(piniaPluginPersistedstate)

// Initialize pinia first so we can use it right away
const app = createApp(App)
app.use(pinia)

// Import the store (must be after pinia initialization)
import { useStore } from './store/store'
const store = useStore()

// Create Vuetify instance with theme from store
const vuetify = createVuetify({
  components,
  directives,
  theme: {
    // Use the theme from the store instead of hardcoded 'light'
    defaultTheme: store.theme,
    themes: {
      light: {
        colors: {
          primary: '#1867C0',
          secondary: '#5CBBF6',
          background: '#FFFFFF',
          surface: '#FBFBFF',
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

// Complete the app initialization
app
  .use(router)
  .use(VueHighlightJS)
  // @ts-expect-error TS doesn't understand axios cache
  .use(VueAxios, axios)
  .use(vuetify)
  .mount('#app')
