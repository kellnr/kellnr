import { type GetterTree } from "vuex";
import { type State } from "./state";

export type Getters = {
    theme(state: State): string
}

export const getters: GetterTree<State, State> & Getters = {
    theme: (state) => {
        return state.theme
    }
}
