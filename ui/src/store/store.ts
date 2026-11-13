import { defineStore } from 'pinia'

export interface State {
    loggedInUser: string | null
    loggedInUserIsAdmin: boolean
    loggedInUserIsReadOnly: boolean
    theme: string
    rememberMe: boolean
    rememberMeUser: string | null
}

export const useStore = defineStore('store', {
    state: (): State => ({
        loggedInUser: null,
        loggedInUserIsAdmin: false,
        loggedInUserIsReadOnly: false,
        // Applies until a user picks a theme, the choice is stored per browser
        theme: 'dark',
        rememberMe: false,
        rememberMeUser: null,
    }),
    getters: {
        loggedIn: (state) => state.loggedInUser !== null,
        isDark: (state) => state.theme === 'dark',
        cargoSmallLogo: () => 'img/cargo-logo-small-light.png',
        kellnrSmallLogo: () => 'img/kellnr-crate.svg',
    },
    actions: {
        login(payload: { "user": string, "is_admin": boolean, "is_read_only": boolean }) {
            this.loggedInUser = payload.user
            this.loggedInUserIsAdmin = payload.is_admin
            this.loggedInUserIsReadOnly = payload.is_read_only
        },
        logout() {
            this.loggedInUser = null
            this.loggedInUserIsAdmin = false
            this.loggedInUserIsReadOnly = false
        },
        toggleTheme() {
            // Toggle theme state - Vuetify and highlight.js themes are handled by ThemeToggle component
            this.theme = this.theme === 'light' ? 'dark' : 'light'
        }
    },
    // Only persist user preferences. Asset paths stay in code so a script on
    // the same origin (e.g. hosted docs) cannot point them at other URLs.
    persist: {
        pick: ['loggedInUser', 'loggedInUserIsAdmin', 'loggedInUserIsReadOnly', 'theme', 'rememberMe', 'rememberMeUser'],
    },
})
