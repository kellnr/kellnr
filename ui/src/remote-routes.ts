export const ADD_TOKEN = "/api/v1/user/add_token";
export const DELETE_TOKEN = (id: number) => `/api/v1/user/delete_token/${id}`;
export const LIST_TOKENS = "/api/v1/user/list_tokens";
export const CHANGE_PWD = "/api/v1/user/change_pwd";
export const LOGIN_STATE = "/api/v1/user/login_state";
export const LOGOUT = "/api/v1/user/logout";
export const ADD_USER = "/api/v1/user/add";
export const DELETE_USER = (name: string) => `/api/v1/user/delete/${name}`;
export const LIST_USERS = "/api/v1/user/list_users";
export const RESET_PWD = (name: string) => `/api/v1/user/reset_pwd/${name}`;
export const LOGIN = "/api/v1/user/login";

export const CRATE_DATA = "/api/v1/ui/crate_data";
export const CRATESIO_DATA = "/api/v1/ui/cratesio_data";
export const CRATES = "/api/v1/ui/crates";
export const CRATE_DELETE_VERSION = "/api/v1/ui/delete_version";
export const CRATE_DELETE_ALL = "/api/v1/ui/delete_crate";
export const VERSION = "/api/v1/ui/version";
export const SETTINGS = "/api/v1/ui/settings";
export const STATISTICS = "/api/v1/ui/statistic";
export const SEARCH = "/api/v1/ui/search";

export const CRATES_IO_INDEX = "/api/v1/cratesio/index";
export const DOCS_BUILD = "/api/v1/docs/build";
export const DOCS_QUEUE = "/api/v1/docs/queue";

// External URL
export const CRATESIO_LINK = (name: string) => `https://crates.io/crates/${name}`;
