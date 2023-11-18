export function kellnr_url(path: string): string {
  const BASE_URL = process.env.NODE_ENV === "development" ? "http://localhost:8000" : window.location.origin;
  return new URL(path, BASE_URL).toString();
}

export const ADD_TOKEN = kellnr_url("/api/v1/user/add_token");
export const DELETE_TOKEN = (id: number) => kellnr_url(`/api/v1/user/delete_token/${id}`);
export const LIST_TOKENS = kellnr_url("/api/v1/user/list_tokens");
export const CHANGE_PWD = kellnr_url("/api/v1/user/change_pwd");
export const LOGIN_STATE = kellnr_url("/api/v1/user/login_state");
export const LOGOUT = kellnr_url("/api/v1/user/logout");
export const ADD_USER = kellnr_url("/api/v1/user/add");
export const DELETE_USER = (name: string) => kellnr_url(`/api/v1/user/delete/${name}`);
export const LIST_USERS = kellnr_url("/api/v1/user/list_users");
export const RESET_PWD = (name: string) => kellnr_url(`/api/v1/user/reset_pwd/${name}`);
export const LOGIN = kellnr_url("/api/v1/user/login");

export const CRATE_DATA = kellnr_url("/api/v1/ui/crate_data");
export const CRATESIO_DATA = kellnr_url("/api/v1/ui/cratesio_data");
export const CRATES = kellnr_url("/api/v1/ui/crates");
export const CRATE_DELETE = kellnr_url("/api/v1/ui/delete_crate");
export const VERSION = kellnr_url("/api/v1/ui/version");
export const SETTINGS = kellnr_url("/api/v1/ui/settings");
export const STATISTICS = kellnr_url("/api/v1/ui/statistic");
export const SEARCH = kellnr_url("/api/v1/ui/search");

export const CRATES_IO_INDEX = kellnr_url("/api/v1/cratesio/index");
export const DOCS_BUILD = kellnr_url("/api/v1/docs/build");
export const DOCS_QUEUE = kellnr_url("/api/v1/docs/queue");

// External URL
export const CRATESIO_LINK = (name: string) => `https://crates.io/crates/${name}`;
