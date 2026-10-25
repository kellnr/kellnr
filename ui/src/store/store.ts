import { defineStore } from 'pinia'

export interface State {
    loggedInUser: string | null
    loggedInUserIsAdmin: boolean
    theme: string
    rememberMe: boolean
    rememberMeUser: string | null
    searchCache: boolean
}

export const useStore = defineStore('store', {
    state: (): State => ({
        loggedInUser: null,
        loggedInUserIsAdmin: false,
        theme: 'light',
        rememberMe: false,
        rememberMeUser: null,
        searchCache: false,
    }),
    getters: {
        loggedIn: (state) => state.loggedInUser !== null,
        isDark: (state) => state.theme === 'dark',
        cargoSmallLogo: () => 'img/cargo-logo-small-light.png',
        kellnrSmallLogo: () => 'img/kellnr-crate.svg',
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
            this.theme = this.theme === 'light' ? 'dark' : 'light'
        }
    },
    // Only persist user preferences. Asset paths stay in code so a script on
    // the same origin (e.g. hosted docs) cannot point them at other URLs.
    persist: {
        pick: ['loggedInUser', 'loggedInUserIsAdmin', 'theme', 'rememberMe', 'rememberMeUser', 'searchCache'],
    },
})
