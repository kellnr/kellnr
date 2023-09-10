import { ActionTypes } from './action-types'
import {State} from "@/store/state";
import {ActionTree} from "vuex";

export interface Actions {
}

export const actions: ActionTree<State, State> & Actions = {
    // No action a.t.m
    // See: https://dev.to/3vilarthas/vuex-typescript-m4j
}