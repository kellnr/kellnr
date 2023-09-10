import { MutationTypes } from "@/store/mutation-types";
export const mutations = {
    [MutationTypes.LOGIN](state, payload) {
        state.loggedIn = true;
        state.loggedInUser = payload["user"];
        state.loggedInUserIsAdmin = payload["is_admin"];
    },
    [MutationTypes.LOGOUT](state, payload) {
        state.loggedIn = false;
        state.loggedInUser = "";
        state.loggedInUserIsAdmin = false;
    },
    [MutationTypes.TOGGLE_THEME](state, payload) {
        if (state.theme === "light") {
            state.theme = "dark";
            state.cargoSmallLogo = "img/cargo-logo-small-dark.png";
            state.kellnrSmallLogo = "img/kellnr-logo-small-dark.png";
        }
        else {
            state.theme = "light";
            state.cargoSmallLogo = "img/cargo-logo-small-light.png";
            state.kellnrSmallLogo = "img/kellnr-logo-small-light.png";
        }
    }
};
//# sourceMappingURL=mutations.js.map