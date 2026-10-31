//! Who sends a request and how they logged in.
//!
//! Every endpoint names the login methods it accepts with its extractor:
//! [`TokenUser`] for a cargo token (or basic auth), [`SessionUser`] for the
//! session cookie of the web UI, [`AnyUser`] for either, and [`AdminUser`]
//! for an admin session. Wrapped in an `Option`, the login is optional.

use std::ops::Deref;

use axum::RequestPartsExt;
use axum::extract::{FromRequestParts, OptionalFromRequestParts};
use axum::http::StatusCode;
use axum::http::request::Parts;
use axum_extra::extract::PrivateCookieJar;
use kellnr_common::account_name::UserName;
use kellnr_common::plain_token::PlainToken;
use kellnr_db::SessionInfo;
use kellnr_db::error::DbError;
use kellnr_settings::constants::COOKIE_SESSION_ID;
use tracing::error;

use crate::authz;
use crate::error::ApiError;
use crate::state::AppStateData;
use crate::token::Token;

/// How a user logged in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// A cargo token, or basic auth credentials if the token is `None`.
    Token(Option<PlainToken>),
    /// The session cookie of the web UI.
    Session,
}

/// A logged in user.
#[derive(Debug, Clone)]
pub struct Identity {
    pub name: UserName,
    pub is_admin: bool,
    pub is_read_only: bool,
    pub source: Source,
}

impl From<Token> for Identity {
    fn from(token: Token) -> Self {
        Self {
            name: token.user,
            is_admin: token.is_admin,
            is_read_only: token.is_read_only,
            source: Source::Token(token.value),
        }
    }
}

impl From<SessionInfo> for Identity {
    fn from(session: SessionInfo) -> Self {
        Self {
            name: session.name,
            is_admin: session.is_admin,
            is_read_only: session.is_read_only,
            source: Source::Session,
        }
    }
}

/// The login methods an endpoint accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accepts {
    Token,
    Session,
    TokenOrSession,
}

/// Identifies the user with one of the `accepts` login methods. Without any
/// credentials, the result is 401. With both, the token is tried first and
/// the session only if the token fails.
pub(crate) async fn identify(
    parts: &mut Parts,
    state: &AppStateData,
    accepts: Accepts,
) -> Result<Identity, StatusCode> {
    match accepts {
        Accepts::Token => from_token(parts, state).await,
        Accepts::Session => from_session(parts, state).await,
        Accepts::TokenOrSession => match from_token(parts, state).await {
            Ok(identity) => Ok(identity),
            Err(_) => from_session(parts, state).await,
        },
    }
}

async fn from_token(parts: &Parts, state: &AppStateData) -> Result<Identity, StatusCode> {
    Token::from_header(&parts.headers, state)
        .await
        .map(Identity::from)
}

async fn from_session(parts: &mut Parts, state: &AppStateData) -> Result<Identity, StatusCode> {
    let jar: PrivateCookieJar = parts
        .extract_with_state(state)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    let cookie = jar.get(COOKIE_SESSION_ID).ok_or(StatusCode::UNAUTHORIZED)?;
    validate_session(state, cookie.value()).await
}

/// Looks up the session of `session_token`. A missing or expired session is
/// a 401, any other failure a 500.
async fn validate_session(
    state: &AppStateData,
    session_token: &str,
) -> Result<Identity, StatusCode> {
    state
        .db
        .validate_session(&PlainToken::from(session_token))
        .await
        .map(Identity::from)
        .map_err(|e| match e {
            DbError::SessionNotFound => StatusCode::UNAUTHORIZED,
            e => {
                error!("Failed to validate session: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        })
}

/// Whether a failed login means that no credentials were sent at all.
fn no_credentials(status: StatusCode) -> bool {
    status == StatusCode::UNAUTHORIZED
}

macro_rules! identity_extractor {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone)]
        pub struct $name(pub Identity);

        impl Deref for $name {
            type Target = Identity;

            fn deref(&self) -> &Identity {
                &self.0
            }
        }
    };
}

identity_extractor!(
    /// A user logged in with a cargo token. Optional, a request without an
    /// `Authorization` header is anonymous, while an invalid token is
    /// rejected.
    TokenUser
);

identity_extractor!(
    /// A user logged in with a session cookie. Optional, a request without a
    /// session cookie is anonymous, while an invalid session is rejected.
    SessionUser
);

identity_extractor!(
    /// A user logged in with a cargo token or a session cookie. Optional,
    /// every failed login is anonymous.
    AnyUser
);

identity_extractor!(
    /// An admin logged in with a session cookie.
    AdminUser
);

impl FromRequestParts<AppStateData> for TokenUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppStateData,
    ) -> Result<Self, Self::Rejection> {
        Ok(Self(identify(parts, state, Accepts::Token).await?))
    }
}

impl OptionalFromRequestParts<AppStateData> for TokenUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppStateData,
    ) -> Result<Option<Self>, Self::Rejection> {
        match identify(parts, state, Accepts::Token).await {
            Ok(identity) => Ok(Some(Self(identity))),
            Err(status) if no_credentials(status) => Ok(None),
            Err(status) => Err(status.into()),
        }
    }
}

impl FromRequestParts<AppStateData> for SessionUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppStateData,
    ) -> Result<Self, Self::Rejection> {
        Ok(Self(identify(parts, state, Accepts::Session).await?))
    }
}

impl OptionalFromRequestParts<AppStateData> for SessionUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppStateData,
    ) -> Result<Option<Self>, Self::Rejection> {
        let jar: PrivateCookieJar = parts
            .extract_with_state(state)
            .await
            .map_err(|_| StatusCode::UNAUTHORIZED)?;
        match jar.get(COOKIE_SESSION_ID) {
            Some(cookie) => Ok(Some(Self(validate_session(state, cookie.value()).await?))),
            None => Ok(None),
        }
    }
}

impl FromRequestParts<AppStateData> for AnyUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppStateData,
    ) -> Result<Self, Self::Rejection> {
        Ok(Self(identify(parts, state, Accepts::TokenOrSession).await?))
    }
}

impl OptionalFromRequestParts<AppStateData> for AnyUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppStateData,
    ) -> Result<Option<Self>, Self::Rejection> {
        Ok(identify(parts, state, Accepts::TokenOrSession)
            .await
            .ok()
            .map(Self))
    }
}

impl FromRequestParts<AppStateData> for AdminUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppStateData,
    ) -> Result<Self, Self::Rejection> {
        let identity = identify(parts, state, Accepts::Session).await?;
        authz::require_admin(&identity)?;
        Ok(Self(identity))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::Router;
    use axum::body::Body;
    use axum::http::{Request, header};
    use axum::routing::get;
    use cookie::Key;
    use kellnr_db::mock::MockDb;
    use kellnr_db::{DbProvider, User};
    use mockall::predicate::eq;
    use tower::ServiceExt;

    use super::*;
    use crate::test_support::{TEST_KEY, encode_cookies, test_state};

    type Result<T = (), E = Box<dyn std::error::Error>> = std::result::Result<T, E>;

    async fn admin_endpoint(_user: AdminUser) {}

    async fn session_endpoint(_user: SessionUser) {}

    async fn optional_session_endpoint(user: Option<SessionUser>) -> String {
        user.map(|u| u.name.to_string()).unwrap_or_default()
    }

    async fn any_endpoint(user: AnyUser) -> String {
        format!("{:?}", user.source)
    }

    async fn optional_any_endpoint(user: Option<AnyUser>) -> String {
        user.map(|u| u.name.to_string()).unwrap_or_default()
    }

    async fn optional_token_endpoint(user: Option<TokenUser>) -> String {
        user.map(|u| u.name.to_string()).unwrap_or_default()
    }

    fn app(db: MockDb) -> Router {
        let db: Arc<dyn DbProvider> = Arc::new(db);
        Router::new()
            .route("/admin", get(admin_endpoint))
            .route("/session", get(session_endpoint))
            .route("/optional_session", get(optional_session_endpoint))
            .route("/any", get(any_endpoint))
            .route("/optional_any", get(optional_any_endpoint))
            .route("/optional_token", get(optional_token_endpoint))
            .with_state(AppStateData {
                db,
                signing_key: Key::from(TEST_KEY),
                ..test_state()
            })
    }

    fn c1234() -> String {
        encode_cookies([(COOKIE_SESSION_ID, "1234")])
    }

    fn session_db(result: fn() -> kellnr_db::provider::DbResult<SessionInfo>) -> MockDb {
        let mut db = MockDb::new();
        db.expect_validate_session()
            .with(eq(PlainToken::from("1234")))
            .returning(move |_| result());
        db
    }

    fn session(is_admin: bool) -> SessionInfo {
        SessionInfo {
            name: UserName::new("user").unwrap(),
            is_admin,
            is_read_only: false,
        }
    }

    fn token_db(db: &mut MockDb) {
        db.expect_get_user_from_token()
            .with(eq(PlainToken::from("token")))
            .returning(|_| {
                Ok(User {
                    name: UserName::new("token_user").unwrap(),
                    ..User::default()
                })
            });
        db.expect_get_user_from_token()
            .with(eq(PlainToken::from("invalid")))
            .returning(|_| Err(DbError::TokenNotFound));
    }

    async fn send(
        app: Router,
        uri: &str,
        headers: &[(header::HeaderName, String)],
    ) -> Result<(StatusCode, String)> {
        let mut request = Request::get(uri);
        for (name, value) in headers {
            request = request.header(name, value);
        }
        let response = app.oneshot(request.body(Body::empty())?).await?;
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await?;
        Ok((status, String::from_utf8(body.to_vec())?))
    }

    #[tokio::test]
    async fn admin_session_is_admin_user() -> Result {
        let app = app(session_db(|| Ok(session(true))));
        let (status, _) = send(app, "/admin", &[(header::COOKIE, c1234())]).await?;
        assert_eq!(status, StatusCode::OK);
        Ok(())
    }

    #[tokio::test]
    async fn normal_session_is_no_admin_user() -> Result {
        let app = app(session_db(|| Ok(session(false))));
        let (status, _) = send(app, "/admin", &[(header::COOKIE, c1234())]).await?;
        assert_eq!(status, StatusCode::FORBIDDEN);
        Ok(())
    }

    #[tokio::test]
    async fn admin_user_needs_a_session() -> Result {
        let (status, _) = send(app(MockDb::new()), "/admin", &[]).await?;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        Ok(())
    }

    #[tokio::test]
    async fn admin_user_with_unknown_session() -> Result {
        let app = app(session_db(|| Err(DbError::SessionNotFound)));
        let (status, _) = send(app, "/admin", &[(header::COOKIE, c1234())]).await?;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        Ok(())
    }

    #[tokio::test]
    async fn session_user_with_session() -> Result {
        let app = app(session_db(|| Ok(session(false))));
        let (status, _) = send(app, "/session", &[(header::COOKIE, c1234())]).await?;
        assert_eq!(status, StatusCode::OK);
        Ok(())
    }

    #[tokio::test]
    async fn session_user_without_session() -> Result {
        let (status, _) = send(app(MockDb::new()), "/session", &[]).await?;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        Ok(())
    }

    #[tokio::test]
    async fn session_user_ignores_tokens() -> Result {
        let mut db = MockDb::new();
        token_db(&mut db);
        let (status, _) = send(
            app(db),
            "/session",
            &[(header::AUTHORIZATION, "token".into())],
        )
        .await?;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        Ok(())
    }

    /// A failing database is not a missing login.
    #[tokio::test]
    async fn session_user_with_database_error() -> Result {
        let app = app(session_db(|| Err(DbError::FailedToCountCrates)));
        let (status, _) = send(app, "/session", &[(header::COOKIE, c1234())]).await?;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        Ok(())
    }

    #[tokio::test]
    async fn session_user_with_unknown_session() -> Result {
        let app = app(session_db(|| Err(DbError::SessionNotFound)));
        let (status, _) = send(app, "/session", &[(header::COOKIE, c1234())]).await?;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        Ok(())
    }

    #[tokio::test]
    async fn optional_session_user_without_session_is_anonymous() -> Result {
        let (status, body) = send(app(MockDb::new()), "/optional_session", &[]).await?;
        assert_eq!((status, body.as_str()), (StatusCode::OK, ""));
        Ok(())
    }

    #[tokio::test]
    async fn optional_session_user_with_unknown_session_is_rejected() -> Result {
        let app = app(session_db(|| Err(DbError::SessionNotFound)));
        let (status, _) = send(app, "/optional_session", &[(header::COOKIE, c1234())]).await?;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        Ok(())
    }

    #[tokio::test]
    async fn any_user_prefers_the_token() -> Result {
        let mut db = session_db(|| Ok(session(false)));
        token_db(&mut db);
        let (status, body) = send(
            app(db),
            "/any",
            &[
                (header::AUTHORIZATION, "token".into()),
                (header::COOKIE, c1234()),
            ],
        )
        .await?;
        assert_eq!(status, StatusCode::OK);
        assert!(body.starts_with("Token("), "{body}");
        Ok(())
    }

    #[tokio::test]
    async fn any_user_falls_back_to_the_session_if_the_token_fails() -> Result {
        let mut db = session_db(|| Ok(session(false)));
        token_db(&mut db);
        let (status, body) = send(
            app(db),
            "/any",
            &[
                (header::AUTHORIZATION, "invalid".into()),
                (header::COOKIE, c1234()),
            ],
        )
        .await?;
        assert_eq!((status, body.as_str()), (StatusCode::OK, "Session"));
        Ok(())
    }

    #[tokio::test]
    async fn any_user_without_login() -> Result {
        let (status, _) = send(app(MockDb::new()), "/any", &[]).await?;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        Ok(())
    }

    #[tokio::test]
    async fn optional_any_user_with_invalid_token_is_anonymous() -> Result {
        let mut db = MockDb::new();
        token_db(&mut db);
        let (status, body) = send(
            app(db),
            "/optional_any",
            &[(header::AUTHORIZATION, "invalid".into())],
        )
        .await?;
        assert_eq!((status, body.as_str()), (StatusCode::OK, ""));
        Ok(())
    }

    #[tokio::test]
    async fn optional_token_user_without_token_is_anonymous() -> Result {
        let (status, body) = send(app(MockDb::new()), "/optional_token", &[]).await?;
        assert_eq!((status, body.as_str()), (StatusCode::OK, ""));
        Ok(())
    }

    #[tokio::test]
    async fn optional_token_user_with_invalid_token_is_rejected() -> Result {
        let mut db = MockDb::new();
        token_db(&mut db);
        let (status, _) = send(
            app(db),
            "/optional_token",
            &[(header::AUTHORIZATION, "invalid".into())],
        )
        .await?;
        assert_eq!(status, StatusCode::FORBIDDEN);
        Ok(())
    }

    #[tokio::test]
    async fn optional_token_user_with_token() -> Result {
        let mut db = MockDb::new();
        token_db(&mut db);
        let (status, body) = send(
            app(db),
            "/optional_token",
            &[(header::AUTHORIZATION, "token".into())],
        )
        .await?;
        assert_eq!((status, body.as_str()), (StatusCode::OK, "token_user"));
        Ok(())
    }
}
