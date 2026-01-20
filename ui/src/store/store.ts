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
    lightBackgroundImage: string
    darkBackgroundImage: string
    currentBackgroundImage: string
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
        searchCache: false,
        lightBackgroundImage: 'img/blob-scene-haikei3.svg',
        darkBackgroundImage: 'img/layered-peaks-haikei.svg',
        currentBackgroundImage: 'img/blob-scene-haikei3.svg', // Default to light
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
            // Toggle theme state - Vuetify and highlight.js themes are handled by ThemeToggle component
            if (this.theme === 'light') {
                this.theme = 'dark'
                this.kellnrSmallLogo = 'img/kellnr-logo-small-dark.png'
                this.currentBackgroundImage = this.darkBackgroundImage
            } else {
                this.theme = 'light'
                this.cargoSmallLogo = 'img/cargo-logo-small-light.png'
                this.kellnrSmallLogo = 'img/kellnr-logo-small-light.png'
                this.currentBackgroundImage = this.lightBackgroundImage
            }
        }
    },
    persist: true
})
