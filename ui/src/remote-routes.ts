export const ADD_TOKEN = "./api/v1/user/add_token";
export const DELETE_TOKEN = (id: number) => `./api/v1/user/delete_token/${id}`;
export const LIST_TOKENS = "./api/v1/user/list_tokens";
export const CHANGE_PWD = "./api/v1/user/change_pwd";
export const LOGIN_STATE = "./api/v1/user/login_state";
export const LOGOUT = "./api/v1/user/logout";
export const ADD_USER = "./api/v1/user/add";
export const DELETE_USER = (name: string) => `./api/v1/user/delete/${encodeURIComponent(name)}`;
export const LIST_USERS = "./api/v1/user/list_users";
export const RESET_PWD = (name: string) => `./api/v1/user/reset_pwd/${encodeURIComponent(name)}`;
export const USER_READ_ONLY = (name: string) => `./api/v1/user/read_only/${encodeURIComponent(name)}`;
export const LOGIN = "./api/v1/user/login";

export const ADD_GROUP = "./api/v1/group/add";
export const DELETE_GROUP = (name: string) => `./api/v1/group/delete/${encodeURIComponent(name)}`;
export const LIST_GROUPS = "./api/v1/group";
export const GROUP_USERS = (group_name: string) => `./api/v1/group/${encodeURIComponent(group_name)}/users`;
export const GROUP_USER = (group_name: string, name: string) => `./api/v1/group/${encodeURIComponent(group_name)}/users/${encodeURIComponent(name)}`;

export const CRATE_USERS = (crate_name: string) => `./api/v1/crate_access/${encodeURIComponent(crate_name)}/users`;
export const CRATE_USER = (crate_name: string, name: string) => `./api/v1/crate_access/${encodeURIComponent(crate_name)}/users/${encodeURIComponent(name)}`;
export const CRATE_ACCESS_DATA = (crate_name: string) => `./api/v1/crate_access/${encodeURIComponent(crate_name)}/access_data`;
export const CRATE_GROUPS = (crate_name: string) => `./api/v1/crate_access/${encodeURIComponent(crate_name)}/groups`;
export const CRATE_GROUP = (crate_name: string, name: string) => `./api/v1/crate_access/${encodeURIComponent(crate_name)}/groups/${encodeURIComponent(name)}`;


export const CRATE_OWNERS = (crate_name: string) => `./api/v1/crates/${encodeURIComponent(crate_name)}/owners`;
export const CRATE_OWNER = (crate_name: string, name: string) => `./api/v1/crates/${encodeURIComponent(crate_name)}/owners/${encodeURIComponent(name)}`;
export const CRATE_OWNERS_SET = (crate_name: string) => `./api/v1/crates/${encodeURIComponent(crate_name)}/owners`;


export const CRATE_DATA = "./api/v1/ui/crate_data";
export const CRATESIO_DATA = "./api/v1/ui/cratesio_data";
export const CRATES = "./api/v1/ui/crates";
export const CRATE_DELETE_VERSION = "./api/v1/ui/delete_version";
export const CRATE_DELETE_ALL = "./api/v1/ui/delete_crate";
export const VERSION = "./api/v1/ui/version";
export const SETTINGS = "./api/v1/ui/settings";
export const STATISTICS = "./api/v1/ui/statistic";
export const SEARCH = "./api/v1/ui/search";

export const DOCS_BUILD = "./api/v1/docs/build";
export const DOCS_QUEUE = "./api/v1/docs/queue";

// External URL
export const CRATESIO_LINK = (name: string) => `https://crates.io/crates/${name}`;
