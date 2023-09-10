// See https://dev.to/3vilarthas/vuex-typescript-m4j for the tutorial used to create this store

import {
    createStore,
    Store as VuexStore,
    CommitOptions,
    DispatchOptions
} from "vuex";
import { State, state} from "@/store/state";
import {Getters, getters} from "@/store/getters";
import { Mutations, mutations} from "@/store/mutations";
import {Actions, actions} from "@/store/actions";
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