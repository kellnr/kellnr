import { defineStore } from 'pinia'
import { BACKEND_URL_PATH_PREFIX } from '../remote-routes'

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
        cargoSmallLogo: `${BACKEND_URL_PATH_PREFIX}/img/cargo-logo-small-light.png`,
          kellnrSmallLogo: `${BACKEND_URL_PATH_PREFIX}/img/kellnr-logo-small-light.png`,
        rememberMe: false,
        rememberMeUser: null,
        searchCache: false
    }),
    getters: {
        loggedIn: (state) => state.loggedInUser !== null
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
                this.kellnrSmallLogo = `${BACKEND_URL_PATH_PREFIX}/img/kellnr-logo-small-dark.png`
            } else {
                this.theme = 'light'
                this.cargoSmallLogo = `${BACKEND_URL_PATH_PREFIX}/img/cargo-logo-small-light.png`,
            this.kellnrSmallLogo = `${BACKEND_URL_PATH_PREFIX}/img/kellnr-logo-small-light.png`
            }
        }
    },
    persist: true
})
