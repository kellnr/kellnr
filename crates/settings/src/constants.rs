use std::time::Duration;

// Minimal size of a crate and its documentation.
pub const MIN_BODY_CRATE_AND_DOC_BYTES: usize = 10;

// Name of the cookie set on the client to identify a current session.
pub const COOKIE_SESSION_ID: &str = "kellnr_session_id";

pub const COOKIE_SESSION_USER: &str = "kellnr_session_user";

pub const COOKIE_OIDC_ID_TOKEN: &str = "kellnr_oidc_id_token";

// Binds an OAuth2 login flow to the browser that started it (login CSRF).
pub const COOKIE_OAUTH2_STATE: &str = "kellnr_oauth2_state";

// Time a user has to complete the OAuth2 login at the identity provider.
pub const OAUTH2_STATE_MAX_AGE: Duration = Duration::from_mins(10);

// Upper bound of a session's lifetime. A configured session age of zero means
// sessions never expire and uses this bound, as do larger configured values.
pub const MAX_SESSION_AGE: Duration = Duration::from_hours(876_000);
