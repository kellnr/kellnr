import {GetterTree} from "vuex";
import {State} from "@/store/state";

export type Getters = {
    theme(state: State): string
}

export const getters: GetterTree<State, State> & Getters = {
    theme: (state) => {
        return state.theme
    }
}