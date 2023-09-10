// See https://dev.to/3vilarthas/vuex-typescript-m4j for the tutorial used to create this store
import { createStore } from "vuex";
import { state } from "@/store/state";
import { getters } from "@/store/getters";
import { mutations } from "@/store/mutations";
import { actions } from "@/store/actions";
import createPersistedState from "vuex-persistedstate";
export const store = createStore({
    state,
    getters,
    mutations,
    actions,
    plugins: [createPersistedState()]
});
//# sourceMappingURL=store.js.map