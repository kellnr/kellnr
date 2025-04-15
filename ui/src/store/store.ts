import { defineStore } from 'pinia'

export interface State {
    loggedInUser: string | null
    loggedInUserIsAdmin: boolean
    theme: string
    cargoSmallLogo: string
    kellnrSmallLogo: string
    rememberMe: boolean
    rememberMeUser: string | null
    searchCache: boolean
}

export const useStore = defineStore('store', {
    state: (): State => ({
        loggedInUser: null,
        loggedInUserIsAdmin: false,
        theme: 'light',
        cargoSmallLogo: 'img/cargo-logo-small-light.png',
        kellnrSmallLogo: 'img/kellnr-logo-small-light.png',
        rememberMe: false,
        rememberMeUser: null,
        searchCache: false
    }),
    getters: {
        loggedIn: (state) => state.loggedInUser !== null,
        isDark: (state) => state.theme === 'dark'
    },
    actions: {
        login(payload: { "user": string, "is_admin": boolean }) {
            this.loggedInUser = payload.user
            this.loggedInUserIsAdmin = payload.is_admin
        },
        logout() {
            this.loggedInUser = null
            this.loggedInUserIsAdmin = false
        },
        toggleTheme() {
            if (this.theme === 'light') {
                this.theme = 'dark'
                this.kellnrSmallLogo = 'img/kellnr-logo-small-dark.png'
                // Update Vuetify theme dynamically
                this.updateVuetifyTheme('dark')
            } else {
                this.theme = 'light'
                this.cargoSmallLogo = 'img/cargo-logo-small-light.png'
                this.kellnrSmallLogo = 'img/kellnr-logo-small-light.png'
                // Update Vuetify theme dynamically
                this.updateVuetifyTheme('light')
            }

            // Toggle highlight.js theme
            this.toggleHighlightTheme()
        },
        updateVuetifyTheme(theme: string) {
            // Access Vuetify instance and update theme
            const vuetify = document.querySelector('html')?.getAttribute('data-vue-app')
                ? (window as any)?.$vuetify
                : null;

            if (vuetify && vuetify.theme) {
                vuetify.theme.global.name.value = theme;
            }
        },
        toggleHighlightTheme() {
            // Toggle between highlight.js themes
            const isDark = this.theme === 'dark';

            // Select all highlight.js style links
            const hlLight = document.querySelector('link[href*="highlight.js/styles/github.css"]');
            const hlDark = document.querySelector('link[href*="highlight.js/styles/github-dark.css"]');

            if (hlLight && hlDark) {
                hlLight.setAttribute('disabled', isDark.toString());
                hlDark.setAttribute('disabled', (!isDark).toString());
            }
        }
    },
    persist: true
})
