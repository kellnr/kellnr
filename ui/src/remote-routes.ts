export const BACKEND_URL_PATH_PREFIX = import.meta.env.BASE_URL.replace(/\/$/, "");

export const ADD_TOKEN = `${BACKEND_URL_PATH_PREFIX}/api/v1/user/add_token`;
export const DELETE_TOKEN = (id: number) => `${BACKEND_URL_PATH_PREFIX}/api/v1/user/delete_token/${id}`;
export const LIST_TOKENS = `${BACKEND_URL_PATH_PREFIX}/api/v1/user/list_tokens`;
export const CHANGE_PWD = `${BACKEND_URL_PATH_PREFIX}/api/v1/user/change_pwd`;
export const LOGIN_STATE = `${BACKEND_URL_PATH_PREFIX}/api/v1/user/login_state`;
export const LOGOUT = `${BACKEND_URL_PATH_PREFIX}/api/v1/user/logout`;
export const ADD_USER = `${BACKEND_URL_PATH_PREFIX}/api/v1/user/add`;
export const DELETE_USER = (name: string) => `${BACKEND_URL_PATH_PREFIX}/api/v1/user/delete/${encodeURIComponent(name)}`;
export const LIST_USERS = `${BACKEND_URL_PATH_PREFIX}/api/v1/user/list_users`;
export const RESET_PWD = (name: string) => `${BACKEND_URL_PATH_PREFIX}/api/v1/user/reset_pwd/${encodeURIComponent(name)}`;
export const USER_READ_ONLY = (name: string) => `${BACKEND_URL_PATH_PREFIX}/api/v1/user/read_only/${encodeURIComponent(name)}`;
export const LOGIN = `${BACKEND_URL_PATH_PREFIX}/api/v1/user/login`;

export const ADD_GROUP = `${BACKEND_URL_PATH_PREFIX}/api/v1/group/add`;
export const DELETE_GROUP = (name: string) => `${BACKEND_URL_PATH_PREFIX}/api/v1/group/delete/${encodeURIComponent(name)}`;
export const LIST_GROUPS = `${BACKEND_URL_PATH_PREFIX}/api/v1/group`;
export const GROUP_USERS = (group_name: string) => `${BACKEND_URL_PATH_PREFIX}/api/v1/group/${encodeURIComponent(group_name)}/users`;
export const GROUP_USER = (group_name: string, name: string) => `${BACKEND_URL_PATH_PREFIX}/api/v1/group/${encodeURIComponent(group_name)}/users/${encodeURIComponent(name)}`;

export const CRATE_USERS = (crate_name: string) => `${BACKEND_URL_PATH_PREFIX}/api/v1/crate_access/${crate_name}/users`;
export const CRATE_USER = (crate_name: string, name:string) => `${BACKEND_URL_PATH_PREFIX}/api/v1/crate_access/${crate_name}/users/${encodeURIComponent(name)}`;
export const CRATE_ACCESS_DATA = (crate_name: string) => `${BACKEND_URL_PATH_PREFIX}/api/v1/crate_access/${crate_name}/access_data`;
export const CRATE_GROUPS = (crate_name: string) => `${BACKEND_URL_PATH_PREFIX}/api/v1/crate_access/${crate_name}/groups`;
export const CRATE_GROUP = (crate_name: string, name: string) => `${BACKEND_URL_PATH_PREFIX}/api/v1/crate_access/${crate_name}/groups/${encodeURIComponent(name)}`;

export const CRATE_DATA = `${BACKEND_URL_PATH_PREFIX}/api/v1/ui/crate_data`;
export const CRATESIO_DATA = `${BACKEND_URL_PATH_PREFIX}/api/v1/ui/cratesio_data`;
export const CRATES = `${BACKEND_URL_PATH_PREFIX}/api/v1/ui/crates`;
export const CRATE_DELETE_VERSION = `${BACKEND_URL_PATH_PREFIX}/api/v1/ui/delete_version`;
export const CRATE_DELETE_ALL = `${BACKEND_URL_PATH_PREFIX}/api/v1/ui/delete_crate`;
export const VERSION = `${BACKEND_URL_PATH_PREFIX}/api/v1/ui/version`;
export const SETTINGS = `${BACKEND_URL_PATH_PREFIX}/api/v1/ui/settings`;
export const STATISTICS = `${BACKEND_URL_PATH_PREFIX}/api/v1/ui/statistic`;
export const SEARCH = `${BACKEND_URL_PATH_PREFIX}/api/v1/ui/search`;

export const DOCS_BUILD = `${BACKEND_URL_PATH_PREFIX}/api/v1/docs/build`;
export const DOCS_QUEUE = `${BACKEND_URL_PATH_PREFIX}/api/v1/docs/queue`;

// External URL
export const CRATESIO_LINK = (name: string) => `https://crates.io/crates/${name}`;
