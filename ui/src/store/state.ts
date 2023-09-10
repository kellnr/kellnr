export const state = {
    loggedIn: false,
    loggedInUser: "",
    loggedInUserIsAdmin: false,
    theme: 'light',
    cargoSmallLogo: "img/cargo-logo-small-light.png",
    kellnrSmallLogo: "img/kellnr-logo-small-light.png",
    rememberMe: false,
    rememberMeUser: "",
}

export type State = typeof state