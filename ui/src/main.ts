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

const axios = setupCache(Axios);
const pinia = createPinia()
pinia.use(piniaPluginPersistedstate)

// Initialize pinia first so we can use it right away
const app = createApp(App)
app.use(pinia)

// Import the store (must be after pinia initialization)
import { useStore } from './store/store'
const store = useStore()

// Kellnr Color Palette
// A cohesive color system for both light and dark themes
const kellnrLightTheme = {
  colors: {
    // Brand colors
    primary: '#1976D2',           // Kellnr blue - main actions, links
    'primary-darken-1': '#1565C0',
    'primary-lighten-1': '#42A5F5',
    secondary: '#7C4DFF',         // Purple accent
    'secondary-darken-1': '#651FFF',
    'secondary-lighten-1': '#B388FF',

    // Backgrounds
    background: '#F5F7FA',        // Page background - soft gray
    surface: '#FFFFFF',           // Cards, elevated surfaces
    'surface-variant': '#EEF2F6', // Subtle card backgrounds
    'surface-bright': '#FFFFFF',

    // Text colors
    'on-background': '#1A2027',   // Primary text on background
    'on-surface': '#1A2027',      // Primary text on surfaces
    'on-surface-variant': '#5A6A7A', // Secondary text
    'on-primary': '#FFFFFF',
    'on-secondary': '#FFFFFF',

    // Semantic colors
    error: '#D32F2F',
    'on-error': '#FFFFFF',
    success: '#388E3C',
    'on-success': '#FFFFFF',
    warning: '#F57C00',
    'on-warning': '#FFFFFF',
    info: '#0288D1',
    'on-info': '#FFFFFF',

    // UI elements
    'outline': '#D0D7DE',         // Borders
    'outline-variant': '#E4E9EE', // Subtle borders

    // Custom Kellnr colors for cards
    'card-blue': '#4A7AB0',       // Muted blue for hero cards
    'card-purple': '#7D5F8F',     // Muted purple for hero cards
    'card-teal': '#4A8A81',       // Muted teal for hero cards
  }
}

const kellnrDarkTheme = {
  colors: {
    // Brand colors
    primary: '#64B5F6',           // Lighter blue for dark mode
    'primary-darken-1': '#42A5F5',
    'primary-lighten-1': '#90CAF9',
    secondary: '#B388FF',         // Lighter purple for dark mode
    'secondary-darken-1': '#7C4DFF',
    'secondary-lighten-1': '#E1BEE7',

    // Backgrounds - based on the wave background colors
    background: '#0D1B2A',        // Deep navy - matches wave bg
    surface: '#1B2838',           // Slightly lighter navy for cards
    'surface-variant': '#243447', // Card hover, subtle elevation
    'surface-bright': '#2D3F52',  // Brighter surface for emphasis

    // Text colors
    'on-background': '#E8EEF4',   // Primary text - high contrast
    'on-surface': '#E8EEF4',      // Primary text on surfaces
    'on-surface-variant': '#A0B0C0', // Secondary text
    'on-primary': '#0D1B2A',
    'on-secondary': '#0D1B2A',

    // Semantic colors
    error: '#EF5350',
    'on-error': '#0D1B2A',
    success: '#66BB6A',
    'on-success': '#0D1B2A',
    warning: '#FFA726',
    'on-warning': '#0D1B2A',
    info: '#29B6F6',
    'on-info': '#0D1B2A',

    // UI elements
    'outline': '#3D5068',         // Borders
    'outline-variant': '#2D3F52', // Subtle borders

    // Custom Kellnr colors for cards
    'card-blue': '#1565C0',       // Rich blue for hero cards
    'card-purple': '#7B1FA2',     // Rich purple for hero cards
    'card-teal': '#00897B',       // Rich teal for hero cards
  }
}

// Create Vuetify instance with theme from store
const vuetify = createVuetify({
  components,
  directives,
  theme: {
    // Use the theme from the store instead of hardcoded 'light'
    defaultTheme: store.theme,
    themes: {
      light: kellnrLightTheme,
      dark: kellnrDarkTheme
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
