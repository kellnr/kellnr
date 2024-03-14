// See https://dev.to/3vilarthas/vuex-typescript-m4j for the tutorial used to create this store

import {
    createStore,
    Store as VuexStore,
    type CommitOptions,
    type DispatchOptions
} from "vuex";
import { type State, state } from "./state";
import { type Getters, getters } from "./getters";
import { type Mutations, mutations } from "./mutations";
import { type Actions, actions } from "./actions";
import createPersistedState from "vuex-persistedstate";

export const store = createStore({
    state,
    getters,
    mutations,
    actions,
    plugins: [createPersistedState()]
})

export type Store = Omit<
    VuexStore<State>,
    'getters' | 'commit' | 'dispatch'
> & {
    commit<K extends keyof Mutations, P extends Parameters<Mutations[K]>[1]>(
        key: K,
        payload: P,
        options?: CommitOptions
    ): ReturnType<Mutations[K]>
} & {
    dispatch<K extends keyof Actions>(
        key: K,
        payload: Parameters<Actions[K]>[1],
        options?: DispatchOptions
    ): ReturnType<Actions[K]>
} & {
    getters: {
        [K in keyof Getters]: ReturnType<Getters[K]>
    }
}
