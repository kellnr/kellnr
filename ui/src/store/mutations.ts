import { MutationTypes} from "@/store/mutation-types";
import { State } from "@/store/state";
import { MutationTree} from "vuex";

export type Mutations<S = State> = {
    [MutationTypes.LOGIN](state: S, payload: {"user": string, "is_admin": boolean}): void,
    [MutationTypes.LOGOUT](state: S, payload: any): void,
    [MutationTypes.TOGGLE_THEME](state: S, payload: any): void,
}

export const mutations: MutationTree<State> & Mutations = {
    [MutationTypes.LOGIN](state, payload: {"user": string, "is_admin": boolean}) {
        state.loggedIn = true
        state.loggedInUser = payload["user"]
        state.loggedInUserIsAdmin = payload["is_admin"]
    },
    [MutationTypes.LOGOUT](state, payload: any) {
        state.loggedIn = false
        state.loggedInUser = ""
        state.loggedInUserIsAdmin = false
    },
    [MutationTypes.TOGGLE_THEME](state, payload: any) {
        if (state.theme === "light") {
            state.theme = "dark"
            //state.cargoSmallLogo = "img/cargo-logo-small-dark.png"
            state.kellnrSmallLogo = "img/kellnr-logo-small-dark.png"
        } else {
            state.theme = "light"
            state.cargoSmallLogo = "img/cargo-logo-small-light.png"
            state.kellnrSmallLogo = "img/kellnr-logo-small-light.png"
        }
    }
}