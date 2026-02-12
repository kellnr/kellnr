// Authentication
export const LOGIN = "./api/v1/auth/login";
export const LOGOUT = "./api/v1/auth/logout";
export const LOGIN_STATE = "./api/v1/auth/state";

// User Management
export const LIST_USERS = "./api/v1/users";
export const ADD_USER = "./api/v1/users";
export const DELETE_USER = (name: string) => `./api/v1/users/${encodeURIComponent(name)}`;
export const RESET_PWD = (name: string) => `./api/v1/users/${encodeURIComponent(name)}/password`;
export const USER_ADMIN = (name: string) => `./api/v1/users/${encodeURIComponent(name)}/admin`;
export const USER_READ_ONLY = (name: string) => `./api/v1/users/${encodeURIComponent(name)}/read-only`;

// Current User (self-service)
export const CHANGE_PWD = "./api/v1/users/me/password";
export const LIST_TOKENS = "./api/v1/users/me/tokens";
export const ADD_TOKEN = "./api/v1/users/me/tokens";
export const DELETE_TOKEN = (id: number) => `./api/v1/users/me/tokens/${id}`;

export const ADD_GROUP = "./api/v1/groups";
export const DELETE_GROUP = (name: string) => `./api/v1/groups/${encodeURIComponent(name)}`;
export const LIST_GROUPS = "./api/v1/groups";
export const GROUP_USERS = (group_name: string) => `./api/v1/groups/${encodeURIComponent(group_name)}/members`;
export const GROUP_USER = (group_name: string, name: string) => `./api/v1/groups/${encodeURIComponent(group_name)}/members/${encodeURIComponent(name)}`;

// Crate Access Control (ACL)
export const CRATE_ACL = (crate_name: string) => `./api/v1/acl/${encodeURIComponent(crate_name)}`;
export const CRATE_ACL_USERS = (crate_name: string) => `./api/v1/acl/${encodeURIComponent(crate_name)}/users`;
export const CRATE_ACL_USER = (crate_name: string, name: string) => `./api/v1/acl/${encodeURIComponent(crate_name)}/users/${encodeURIComponent(name)}`;
export const CRATE_ACL_GROUPS = (crate_name: string) => `./api/v1/acl/${encodeURIComponent(crate_name)}/groups`;
export const CRATE_ACL_GROUP = (crate_name: string, name: string) => `./api/v1/acl/${encodeURIComponent(crate_name)}/groups/${encodeURIComponent(name)}`;

export const CRATE_OWNERS = (crate_name: string) => `./api/v1/crates/${encodeURIComponent(crate_name)}/owners`;
export const CRATE_OWNER = (crate_name: string, name: string) => `./api/v1/crates/${encodeURIComponent(crate_name)}/owners/${encodeURIComponent(name)}`;

export const CRATE_DATA = "./api/v1/ui/crate_data";
export const CRATESIO_DATA = "./api/v1/ui/cratesio_data";
export const CRATES = "./api/v1/ui/crates";
export const CRATE_DELETE = (name: string) => `./api/v1/ui/crates/${encodeURIComponent(name)}`;
export const CRATE_DELETE_VERSION = (name: string, version: string) =>
  `./api/v1/ui/crates/${encodeURIComponent(name)}/${encodeURIComponent(version)}`;
export const VERSION = "./api/v1/ui/version";
export const SETTINGS = "./api/v1/ui/settings";
export const DOCS_ENABLED = "./api/v1/ui/docs_enabled";
export const STATISTICS = "./api/v1/ui/statistics";
export const SEARCH = "./api/v1/ui/search";

export const DOCS_BUILDS = "./api/v1/docs/builds";

// OAuth2/OIDC
export const OAUTH2_CONFIG = "./api/v1/oauth2/config";
export const OAUTH2_LOGIN = "./api/v1/oauth2/login";

// Toolchain Distribution Server
export const TOOLCHAIN_LIST = "./api/v1/toolchains";
export const TOOLCHAIN_DELETE = (name: string, version: string) =>
  `./api/v1/toolchains/${encodeURIComponent(name)}/${encodeURIComponent(version)}`;
export const TOOLCHAIN_DELETE_TARGET = (name: string, version: string, target: string) =>
  `./api/v1/toolchains/${encodeURIComponent(name)}/${encodeURIComponent(version)}/targets/${encodeURIComponent(target)}`;
export const TOOLCHAIN_CHANNELS = "./api/v1/toolchains/channels";
export const TOOLCHAIN_SET_CHANNEL = (channel: string) =>
  `./api/v1/toolchains/channels/${encodeURIComponent(channel)}`;

// External URL
export const CRATESIO_LINK = (name: string) => `https://crates.io/crates/${name}`;
