export function kellnr_url(path: string): string {
  const BASE_URL = process.env.NODE_ENV === "development" ? "http://localhost:8000" : window.location.origin;
  return new URL(path, BASE_URL).toString();
}

export const ADD_TOKEN = kellnr_url("/user/add_token");
export const DELETE_TOKEN = (id: number) => kellnr_url(`/user/delete_token/${id}`);
export const LIST_TOKENS = kellnr_url("/user/list_tokens");
export const CHANGE_PWD = kellnr_url("/user/changepwd");
export const LOGIN_STATE = kellnr_url("/user/login_state");
export const LOGOUT = kellnr_url("/user/logout");
export const ADD_USER = kellnr_url("/user/add");
export const DELETE_USER = (name: string) => kellnr_url(`/user/delete/${name}`);
export const LIST_USERS = kellnr_url("/user/list_users");
export const RESET_PWD = (name: string) => kellnr_url(`/user/resetpwd/${name}`);
export const LOGIN = kellnr_url("/user/login");

export const CRATE_DATA = kellnr_url("/crate_data");
export const CRATESIO_DATA = kellnr_url("/cratesio_data");
export const CRATES = kellnr_url("/crates");
export const CRATE_DELETE = kellnr_url("/delete_crate");
export const VERSION = kellnr_url("/version");
export const SETTINGS = kellnr_url("/settings");
export const STATISTICS = kellnr_url("/statistic");
export const SEARCH = kellnr_url("/search");

export const CRATES_IO_INDEX = kellnr_url("/api/v1/cratesio/index");
export const DOCS_BUILD = kellnr_url("/api/v1/docs/build");
export const DOCS_QUEUE = kellnr_url("/api/v1/docs/queue");

// External URL
export const CRATESIO_LINK = (name: string) => `https://crates.io/crates/${name}`;
