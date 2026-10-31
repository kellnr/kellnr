use axum::http::StatusCode;
use axum_extra::extract::PrivateCookieJar;
use axum_extra::extract::cookie::Cookie;
use cookie::{SameSite, time};
use kellnr_common::account_name::UserName;
use kellnr_common::plain_token::PlainToken;
use kellnr_common::util::generate_rand_string;
use kellnr_http::error::ApiError;
use kellnr_http::state::AppStateData;
use kellnr_settings::constants::COOKIE_SESSION_ID;
use time::Duration;
use tracing::error;

/// Creates a new session for the user and returns a cookie jar with the session cookie set.
/// Generates a token, persists it via db, and adds the cookie using `app_state` settings.
pub(crate) async fn create_session_jar(
    cookies: PrivateCookieJar,
    app_state: &AppStateData,
    username: &UserName,
) -> Result<PrivateCookieJar, ApiError> {
    // 32 alphanumeric chars (~190 bits) to match the API token length, well
    // above the ~128-bit session-id recommendation.
    let session_token = PlainToken::from(generate_rand_string(32));
    app_state
        .db
        .add_session_token(username, &session_token)
        .await
        .map_err(|e| {
            error!("Failed to create session: {e}");
            ApiError::from(StatusCode::INTERNAL_SERVER_ERROR)
        })?;
    // `Secure` follows `origin.protocol` unless `origin.cookie_secure` forces
    // it, since browsers drop secure cookies on plain-HTTP deployments.
    let secure = app_state.settings.origin.secure_cookies();
    Ok(cookies.add(session_cookie(
        COOKIE_SESSION_ID,
        session_token.into_secret(),
        app_state.settings.registry.session_age().cookie_max_age(),
        secure,
    )))
}

pub(crate) fn session_cookie(
    name: &'static str,
    value: String,
    max_age: std::time::Duration,
    secure: bool,
) -> Cookie<'static> {
    // `max_age` is bounded by `MAX_SESSION_AGE`, which always fits.
    let max_age = Duration::try_from(max_age).unwrap_or(Duration::MAX);
    Cookie::build((name, value))
        .max_age(max_age)
        .same_site(SameSite::Strict)
        .http_only(true)
        .secure(secure)
        .path("/")
        .into()
}

#[cfg(test)]
mod session_cookie_tests {
    use std::sync::Arc;

    use axum::http::HeaderMap;
    use axum_extra::extract::cookie::Key;
    use kellnr_db::mock::MockDb;
    use kellnr_settings::constants::MAX_SESSION_AGE;
    use kellnr_settings::{Protocol, Settings};

    use super::*;

    async fn new_session_cookie(settings: Settings) -> Cookie<'static> {
        let mut db = MockDb::new();
        db.expect_add_session_token().returning(|_, _| Ok(()));
        let state = AppStateData {
            db: Arc::new(db),
            settings: Arc::new(settings),
            ..kellnr_http::test_support::test_state()
        };
        let jar = PrivateCookieJar::from_headers(&HeaderMap::new(), Key::generate());

        let jar = create_session_jar(jar, &state, &UserName::new("user").unwrap())
            .await
            .unwrap();

        jar.get(COOKIE_SESSION_ID).unwrap()
    }

    async fn session_cookie_secure(protocol: Protocol, cookie_secure: Option<bool>) -> bool {
        let mut settings = Settings::clone(&kellnr_http::test_support::test_state().settings);
        settings.origin.protocol = protocol;
        settings.origin.cookie_secure = cookie_secure;
        new_session_cookie(settings).await.secure().unwrap_or(false)
    }

    async fn session_cookie_max_age(session_age_seconds: u64) -> Option<Duration> {
        let mut settings = Settings::clone(&kellnr_http::test_support::test_state().settings);
        settings.registry.session_age_seconds = session_age_seconds;
        new_session_cookie(settings).await.max_age()
    }

    #[tokio::test]
    async fn cookie_lives_as_long_as_the_session() {
        assert_eq!(
            session_cookie_max_age(3600).await,
            Some(Duration::seconds(3600))
        );
    }

    /// A session age of zero means sessions never expire. A cookie with
    /// `Max-Age=0` would be dropped by the browser right away.
    #[tokio::test]
    async fn cookie_of_a_session_that_never_expires_is_kept() {
        let max_age = Duration::try_from(MAX_SESSION_AGE).unwrap();
        assert_eq!(session_cookie_max_age(0).await, Some(max_age));
        assert_eq!(session_cookie_max_age(u64::MAX).await, Some(max_age));
    }

    #[tokio::test]
    async fn secure_follows_protocol_by_default() {
        assert!(session_cookie_secure(Protocol::Https, None).await);
        assert!(!session_cookie_secure(Protocol::Http, None).await);
    }

    /// Behind a TLS-terminating proxy the configured protocol is http, but
    /// the operator can still force the `Secure` flag.
    #[tokio::test]
    async fn cookie_secure_setting_overrides_protocol() {
        assert!(session_cookie_secure(Protocol::Http, Some(true)).await);
        assert!(!session_cookie_secure(Protocol::Https, Some(false)).await);
    }
}
