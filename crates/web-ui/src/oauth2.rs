//! `OAuth2`/`OpenID` Connect web routes for Kellnr
//!
//! Provides the following endpoints:
//! - `/api/v1/oauth2/config` - GET - Returns `OAuth2` configuration for the UI
//! - `/api/v1/oauth2/login` - GET - Initiates `OAuth2` flow, redirects to provider
//! - `/api/v1/oauth2/callback` - GET - Handles callback from provider

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::Redirect;
use axum::{Extension, Json};
use axum_extra::extract::PrivateCookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use kellnr_common::account_name::UserName;
use kellnr_db::{User, UserFlags};
use kellnr_http::error::ApiError;
use kellnr_http::state::{AppState, AppStateData, DbState, SettingsState};
use kellnr_oauth2::{OAuth2Handler, UserInfo, generate_unique_username};
use kellnr_settings::UrlPathPrefix;
use kellnr_settings::constants::{COOKIE_OAUTH2_STATE, COOKIE_OIDC_ID_TOKEN, OAUTH2_STATE_MAX_AGE};
use serde::{Deserialize, Serialize};
use tracing::{error, trace, warn};
use utoipa::ToSchema;

use crate::session::{create_session_jar, session_cookie};

/// Type alias for the `OAuth2` handler extension
pub type OAuth2Ext = Extension<Option<Arc<OAuth2Handler>>>;

/// `OAuth2` configuration response for the UI
#[derive(Debug, Serialize, ToSchema)]
pub struct OAuth2Config {
    /// Whether `OAuth2` authentication is enabled
    pub enabled: bool,
    /// Text to display on the login button
    pub button_text: String,
    /// Whether local password login is disabled (SSO-only)
    pub enforced: bool,
    /// Whether the login page should redirect straight to the SSO provider
    pub auto_redirect: bool,
}

/// Query parameters received in the `OAuth2` callback. The provider sends
/// either `code` on success or `error` if the login failed or was cancelled.
#[derive(Debug, Deserialize, ToSchema, utoipa::IntoParams)]
pub struct CallbackParams {
    /// Authorization code from the provider
    pub code: Option<String>,
    /// CSRF protection state
    pub state: Option<String>,
    /// Error code from the provider, e.g. `access_denied`
    pub error: Option<String>,
    /// Human readable error from the provider
    pub error_description: Option<String>,
}

/// The `OAuth2` callback query, read from [`CallbackParams`]. An `error`
/// wins over a `code`; a query with neither `error` nor `code` and `state`
/// is rejected with 400.
#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(try_from = "CallbackParams")]
pub enum CallbackQuery {
    /// The user logged in at the provider.
    Success { code: String, state: String },
    /// The login failed or was cancelled at the provider.
    ProviderError {
        error: String,
        error_description: Option<String>,
        state: Option<String>,
    },
}

impl TryFrom<CallbackParams> for CallbackQuery {
    type Error = &'static str;

    fn try_from(params: CallbackParams) -> Result<Self, Self::Error> {
        match params {
            CallbackParams {
                error: Some(error),
                error_description,
                state,
                ..
            } => Ok(Self::ProviderError {
                error,
                error_description,
                state,
            }),
            CallbackParams {
                code: Some(code),
                state: Some(state),
                ..
            } => Ok(Self::Success { code, state }),
            _ => Err("OAuth2 callback without code or state"),
        }
    }
}

/// Get `OAuth2` configuration for the UI
///
/// Returns whether `OAuth2` is enabled and the button text to display.
/// This endpoint is always accessible (no auth required).
#[utoipa::path(
    get,
    path = "/config",
    tag = "oauth2",
    responses(
        (status = 200, description = "OAuth2 configuration", body = OAuth2Config)
    )
)]
#[allow(clippy::unused_async)]
pub async fn get_config(State(settings): SettingsState) -> Json<OAuth2Config> {
    Json(OAuth2Config {
        enabled: settings.oauth2.enabled,
        button_text: settings.oauth2.button_text.clone(),
        enforced: settings.oauth2.enforced,
        auto_redirect: settings.oauth2.auto_redirect,
    })
}

/// Initiate `OAuth2` login flow
///
/// This endpoint:
/// 1. Generates PKCE challenge and state for CSRF protection
/// 2. Stores state/PKCE/nonce in the database and binds the state to the
///    browser with a short-lived encrypted cookie
/// 3. Redirects the user to the `OAuth2` provider's authorization endpoint
#[utoipa::path(
    get,
    path = "/login",
    tag = "oauth2",
    responses(
        (status = 302, description = "Redirect to OAuth2 provider"),
        (status = 404, description = "OAuth2 not enabled")
    )
)]
pub async fn login(
    cookies: PrivateCookieJar,
    State(db): DbState,
    State(settings): SettingsState,
    Extension(oauth2_handler): OAuth2Ext,
) -> Result<(PrivateCookieJar, Redirect), ApiError> {
    trace!("OAuth2 login initiated");

    // Check if OAuth2 is enabled
    let handler = oauth2_handler.as_ref().ok_or_else(|| {
        warn!("OAuth2 login attempted but OAuth2 is not enabled");
        ApiError::from(StatusCode::NOT_FOUND)
    })?;

    // Generate authorization URL with PKCE
    let auth_request = handler.generate_auth_url();

    // Store state in database for verification in callback
    db.store_oauth2_state(
        &auth_request.state,
        &auth_request.pkce_verifier,
        &auth_request.nonce,
    )
    .await
    .map_err(|e| {
        error!("Failed to store OAuth2 state: {}", e);
        ApiError::from(StatusCode::INTERNAL_SERVER_ERROR)
    })?;

    let jar = cookies.add(state_cookie(
        auth_request.state,
        settings.origin.secure_cookies(),
    ));

    // Redirect to provider
    Ok((jar, Redirect::to(auth_request.auth_url.as_str())))
}

/// Cookie binding the `OAuth2` flow to the browser that started it. Without
/// it, an attacker could make a victim's browser complete the attacker's own
/// login (login CSRF). `SameSite=Lax` because a strict cookie would not be
/// sent on the redirect back from the identity provider.
fn state_cookie(state: String, secure: bool) -> Cookie<'static> {
    let max_age = time::Duration::try_from(OAUTH2_STATE_MAX_AGE).unwrap_or(time::Duration::ZERO);
    Cookie::build((COOKIE_OAUTH2_STATE, state))
        .max_age(max_age)
        .same_site(SameSite::Lax)
        .http_only(true)
        .secure(secure)
        .path("/")
        .into()
}

/// Checks that the callback belongs to the login flow started by this
/// browser and removes the single-use state cookie.
fn take_state_cookie(
    jar: PrivateCookieJar,
    query_state: &str,
) -> Result<PrivateCookieJar, ApiError> {
    let matches = jar
        .get(COOKIE_OAUTH2_STATE)
        .is_some_and(|c| c.value() == query_state);
    if !matches {
        warn!("OAuth2 callback state does not match the state cookie of this browser");
        return Err(ApiError::from(StatusCode::BAD_REQUEST));
    }
    Ok(jar.remove(Cookie::build((COOKIE_OAUTH2_STATE, "")).path("/")))
}

/// Handle `OAuth2` callback from the provider
///
/// This endpoint:
/// 1. Validates the CSRF state against the database and the browser cookie
/// 2. Exchanges the authorization code for tokens
/// 3. Validates the ID token
/// 4. Creates or links the user account
/// 5. Creates a session and sets the session cookie
/// 6. Redirects to the UI
#[utoipa::path(
    get,
    path = "/callback",
    tag = "oauth2",
    params(CallbackParams),
    responses(
        (status = 302, description = "Redirect to UI after successful login or to the login page on a provider error"),
        (status = 400, description = "Invalid state or missing code"),
        (status = 401, description = "Token exchange failed"),
        (status = 403, description = "User not found and auto-provisioning disabled"),
        (status = 404, description = "OAuth2 not enabled")
    )
)]
pub async fn callback(
    cookies: PrivateCookieJar,
    Query(query): Query<CallbackQuery>,
    State(app_state): AppState,
    Extension(oauth2_handler): OAuth2Ext,
) -> Result<(PrivateCookieJar, Redirect), ApiError> {
    let (code, query_state) = match query {
        CallbackQuery::Success { code, state } => {
            trace!(state = ?state, "OAuth2 callback received");
            (code, state)
        }
        // The login was cancelled or rejected at the provider.
        CallbackQuery::ProviderError {
            error,
            error_description,
            state,
        } => {
            trace!(state = ?state, "OAuth2 callback received");
            warn!("OAuth2 provider returned an error: {error} - {error_description:?}");
            let cookies = cookies.remove(Cookie::build((COOKIE_OAUTH2_STATE, "")).path("/"));
            let redirect = login_error_redirect(
                &app_state.settings.origin.path_prefix(),
                error_description.as_deref().unwrap_or(&error),
            );
            return Ok((cookies, redirect));
        }
    };

    // Check if OAuth2 is enabled
    let handler = oauth2_handler.as_ref().ok_or_else(|| {
        warn!("OAuth2 callback received but OAuth2 is not enabled");
        ApiError::from(StatusCode::NOT_FOUND)
    })?;

    let cookies = take_state_cookie(cookies, &query_state)?;

    // Retrieve and validate state from database
    let state_data = app_state
        .db
        .get_and_delete_oauth2_state(&query_state)
        .await
        .map_err(|e| {
            error!("Failed to retrieve OAuth2 state: {}", e);
            ApiError::from(StatusCode::INTERNAL_SERVER_ERROR)
        })?
        .ok_or_else(|| {
            warn!("OAuth2 callback with invalid or expired state: {query_state}");
            ApiError::from(StatusCode::BAD_REQUEST)
        })?;

    // Exchange code for tokens and validate
    let token_result = handler
        .exchange_and_validate(&code, &state_data.pkce_verifier, &state_data.nonce)
        .await
        .map_err(|e| {
            error!("Failed to exchange OAuth2 code: {}", e);
            ApiError::from(StatusCode::UNAUTHORIZED)
        })?;

    // Extract user information from token
    let user_info = handler.extract_user_info(&token_result);
    let issuer = handler.issuer_url();

    trace!(
        "OAuth2 authentication successful for subject: {}, email: {:?}",
        user_info.subject, user_info.email
    );

    // Look up or create user
    #[allow(clippy::single_match_else)]
    let user = match app_state
        .db
        .get_user_by_oauth2_identity(issuer, &user_info.subject)
        .await
        .map_err(|e| {
            error!("Failed to look up OAuth2 identity: {}", e);
            ApiError::from(StatusCode::INTERNAL_SERVER_ERROR)
        })? {
        Some(user) => {
            trace!("Found existing user '{}' for OAuth2 identity", user.name);
            // Re-sync admin / read-only state from the IdP on every login so
            // that changes to the user's group membership take effect. Only
            // flags whose group claim is configured are governed by the IdP;
            // an unconfigured claim leaves the existing DB value untouched, so
            // that admins are not silently demoted and manual changes stick.
            sync_oauth2_privileges(
                &app_state,
                handler.admin_claim_configured(),
                handler.read_only_claim_configured(),
                user,
                &user_info,
            )
            .await?
        }
        None => {
            // Check if auto-provisioning is enabled
            if !handler.settings().auto_provision_users {
                warn!(
                    "OAuth2 user not found and auto-provisioning is disabled: {}",
                    user_info.subject
                );
                return Err(ApiError::from(StatusCode::FORBIDDEN));
            }

            // Generate unique username
            let username = generate_unique_username(&user_info, |name| {
                let db = app_state.db.clone();
                async move {
                    db.is_username_available(&UserName::from_unchecked(name))
                        .await
                }
            })
            .await
            .map_err(|e| {
                error!("Failed to check if a user name is available: {e}");
                ApiError::from(StatusCode::INTERNAL_SERVER_ERROR)
            })?;
            let username = UserName::from_unchecked(username);

            trace!(
                "Creating new OAuth2 user '{}' (admin: {}, read_only: {})",
                username, user_info.is_admin, user_info.is_read_only
            );

            // Create new user with OAuth2 identity
            app_state
                .db
                .create_oauth2_user(
                    &username,
                    issuer,
                    &user_info.subject,
                    user_info.email.clone(),
                    UserFlags {
                        admin: user_info.is_admin,
                        read_only: user_info.is_read_only,
                    },
                )
                .await
                .map_err(|e| {
                    error!("Failed to create OAuth2 user: {}", e);
                    ApiError::from(StatusCode::INTERNAL_SERVER_ERROR)
                })?
        }
    };

    let jar = create_session_jar(cookies, &app_state, &user.name).await?;
    let jar = jar.add(session_cookie(
        COOKIE_OIDC_ID_TOKEN,
        token_result.id_token.clone(),
        app_state.settings.registry.session_age().cookie_max_age(),
        app_state.settings.origin.secure_cookies(),
    ));
    trace!("Created session for OAuth2 user: {}", user.name);

    // Redirect to UI root
    let base_path = format!("{}/", app_state.settings.origin.path_prefix());
    Ok((jar, Redirect::to(&base_path)))
}

/// Re-sync an existing user's admin / read-only state from the current `IdP`
/// token claims.
///
/// A flag is only updated when its group claim is configured (the `IdP` is then
/// authoritative for it); an unconfigured claim leaves the existing value
/// untouched, so unconfigured deployments never demote users and manual admin
/// changes are preserved. When anything changed, the auth-token cache is
/// invalidated so existing cargo tokens immediately reflect the new
/// privileges; session auth reads the DB per request and needs no
/// invalidation.
async fn sync_oauth2_privileges(
    app_state: &AppStateData,
    admin_governed: bool,
    read_only_governed: bool,
    mut user: User,
    user_info: &UserInfo,
) -> Result<User, ApiError> {
    let mut changed = false;

    if admin_governed && user.is_admin != user_info.is_admin {
        app_state
            .db
            .change_admin_state(&user.name, user_info.is_admin)
            .await
            .map_err(|e| {
                error!(
                    "Failed to sync OAuth2 admin state for '{}': {}",
                    user.name, e
                );
                ApiError::from(StatusCode::INTERNAL_SERVER_ERROR)
            })?;
        user.is_admin = user_info.is_admin;
        changed = true;
    }

    if read_only_governed && user.is_read_only != user_info.is_read_only {
        app_state
            .db
            .change_read_only_state(&user.name, user_info.is_read_only)
            .await
            .map_err(|e| {
                error!(
                    "Failed to sync OAuth2 read-only state for '{}': {}",
                    user.name, e
                );
                ApiError::from(StatusCode::INTERNAL_SERVER_ERROR)
            })?;
        user.is_read_only = user_info.is_read_only;
        changed = true;
    }

    if changed {
        app_state.token_cache.invalidate_all();
        trace!(
            "Synced OAuth2 privileges for '{}' (admin: {}, read_only: {})",
            user.name, user.is_admin, user.is_read_only
        );
    }

    Ok(user)
}

/// Query parameters of the `OAuth2` error callback
#[derive(Debug, Deserialize, ToSchema, utoipa::IntoParams)]
pub struct ErrorQuery {
    pub error: String,
    #[serde(default)]
    pub error_description: Option<String>,
}

/// Error callback for `OAuth2` flow
///
/// Handles errors from the `OAuth2` provider (e.g., user cancelled, access denied)
#[utoipa::path(
    get,
    path = "/error",
    tag = "oauth2",
    params(ErrorQuery),
    responses(
        (status = 302, description = "Redirect to login page with error")
    )
)]
#[allow(clippy::unused_async)]
pub async fn error_callback(
    State(settings): SettingsState,
    Query(query): Query<ErrorQuery>,
) -> Redirect {
    warn!(
        "OAuth2 error callback: {} - {:?}",
        query.error, query.error_description
    );

    let error_msg = query.error_description.as_deref().unwrap_or(&query.error);
    login_error_redirect(&settings.origin.path_prefix(), error_msg)
}

/// Redirect to the login page of the UI, which shows `error`.
fn login_error_redirect(path_prefix: &UrlPathPrefix, error: &str) -> Redirect {
    let encoded_error = urlencoding::encode(error);
    Redirect::to(&format!("{path_prefix}/login?error={encoded_error}"))
}

#[cfg(test)]
mod sync_tests {
    use std::sync::Arc;

    use kellnr_db::mock::MockDb;
    use mockall::predicate::eq;

    use super::*;

    fn user(is_admin: bool, is_read_only: bool) -> User {
        User {
            id: kellnr_db::UserId::new(1),
            name: UserName::new("alice").unwrap(),
            pwd: String::new(),
            salt: String::new(),
            is_admin,
            is_read_only,
            created: String::new(),
        }
    }

    fn user_info(is_admin: bool, is_read_only: bool) -> UserInfo {
        UserInfo {
            subject: "sub".to_string(),
            email: None,
            preferred_username: None,
            groups: vec![],
            is_admin,
            is_read_only,
        }
    }

    // `MockDb` panics on any method call without a matching expectation, so a
    // mock with no expectations asserts that the DB is never written to.
    fn state(db: MockDb) -> AppStateData {
        AppStateData {
            db: Arc::new(db),
            ..kellnr_http::test_support::test_state()
        }
    }

    #[tokio::test]
    async fn read_only_governed_promotes_user() {
        let mut db = MockDb::new();
        db.expect_change_read_only_state()
            .with(eq(UserName::new("alice").unwrap()), eq(true))
            .times(1)
            .returning(|_, _| Ok(()));

        let result = sync_oauth2_privileges(
            &state(db),
            false,
            true,
            user(false, false),
            &user_info(false, true),
        )
        .await
        .unwrap();

        assert!(result.is_read_only);
    }

    #[tokio::test]
    async fn admin_governed_demotes_user() {
        let mut db = MockDb::new();
        db.expect_change_admin_state()
            .with(eq(UserName::new("alice").unwrap()), eq(false))
            .times(1)
            .returning(|_, _| Ok(()));

        let result = sync_oauth2_privileges(
            &state(db),
            true,
            false,
            user(true, false),
            &user_info(false, false),
        )
        .await
        .unwrap();

        assert!(!result.is_admin);
    }

    #[tokio::test]
    async fn unconfigured_claim_leaves_user_untouched() {
        // Claims not governed by the IdP: the differing claim values must be
        // ignored and no DB write performed (mock has no expectations).
        let result = sync_oauth2_privileges(
            &state(MockDb::new()),
            false,
            false,
            user(true, false),
            &user_info(false, true),
        )
        .await
        .unwrap();

        assert!(result.is_admin);
        assert!(!result.is_read_only);
    }

    #[tokio::test]
    async fn governed_but_unchanged_is_noop() {
        // Governed but values already match: no DB write expected.
        let result = sync_oauth2_privileges(
            &state(MockDb::new()),
            true,
            true,
            user(true, true),
            &user_info(true, true),
        )
        .await
        .unwrap();

        assert!(result.is_admin);
        assert!(result.is_read_only);
    }
}

#[cfg(test)]
mod state_cookie_tests {
    use axum::http::{HeaderMap, header};
    use axum_extra::extract::cookie::Key;
    use kellnr_http::test_support::{TEST_KEY, encode_cookies};

    use super::*;

    fn jar_with_state(state: &'static str) -> PrivateCookieJar {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            encode_cookies([(COOKIE_OAUTH2_STATE, state)])
                .parse()
                .unwrap(),
        );
        PrivateCookieJar::from_headers(&headers, Key::from(TEST_KEY))
    }

    fn empty_jar() -> PrivateCookieJar {
        PrivateCookieJar::from_headers(&HeaderMap::new(), Key::from(TEST_KEY))
    }

    #[test]
    fn state_cookie_attributes() {
        let cookie = state_cookie("abc".to_string(), true);

        assert_eq!(cookie.name(), COOKIE_OAUTH2_STATE);
        assert_eq!(cookie.value(), "abc");
        // Lax, not Strict: the cookie must survive the redirect back from
        // the identity provider.
        assert_eq!(cookie.same_site(), Some(SameSite::Lax));
        assert_eq!(cookie.http_only(), Some(true));
        assert_eq!(cookie.secure(), Some(true));
        assert_eq!(cookie.path(), Some("/"));
        assert_eq!(cookie.max_age(), Some(time::Duration::minutes(10)));
    }

    #[test]
    fn state_cookie_not_secure_over_http() {
        assert_eq!(state_cookie("abc".to_string(), false).secure(), Some(false));
    }

    #[test]
    fn matching_state_is_accepted_and_cookie_removed() {
        let jar = take_state_cookie(jar_with_state("abc"), "abc").unwrap();

        assert!(jar.get(COOKIE_OAUTH2_STATE).is_none());
    }

    /// A callback carrying a state the browser never started, for example
    /// one crafted by an attacker for their own account, must be rejected.
    #[test]
    fn mismatching_state_is_rejected() {
        let result = take_state_cookie(jar_with_state("abc"), "attacker");

        assert!(
            result
                .as_ref()
                .is_err_and(|e| e.status() == StatusCode::BAD_REQUEST)
        );
    }

    #[test]
    fn missing_state_cookie_is_rejected() {
        let result = take_state_cookie(empty_jar(), "abc");

        assert!(
            result
                .as_ref()
                .is_err_and(|e| e.status() == StatusCode::BAD_REQUEST)
        );
    }
}

#[cfg(test)]
mod callback_tests {
    use axum::http::{HeaderMap, header};
    use axum::response::IntoResponse;
    use axum_extra::extract::cookie::Key;
    use kellnr_http::test_support::TEST_KEY;
    use kellnr_settings::Settings;

    use super::*;

    fn state_with_origin_path(path: &str) -> AppStateData {
        let mut settings = Settings::clone(&kellnr_http::test_support::test_state().settings);
        settings.origin.path = path.to_string();
        AppStateData {
            settings: Arc::new(settings),
            ..kellnr_http::test_support::test_state()
        }
    }

    fn location(response: impl IntoResponse) -> String {
        response.into_response().headers()[header::LOCATION]
            .to_str()
            .unwrap()
            .to_string()
    }

    fn parse_query(query: &str) -> Option<CallbackQuery> {
        let uri: axum::http::Uri = format!("/callback?{query}").parse().unwrap();
        Query::<CallbackQuery>::try_from_uri(&uri)
            .ok()
            .map(|Query(query)| query)
    }

    #[test]
    fn callback_query_with_code_and_state_is_success() {
        assert_eq!(
            parse_query("code=c&state=s&unknown=x"),
            Some(CallbackQuery::Success {
                code: "c".to_string(),
                state: "s".to_string()
            })
        );
    }

    #[test]
    fn callback_query_with_error_is_provider_error() {
        assert_eq!(
            parse_query("error=access_denied&error_description=User%20cancelled&state=s"),
            Some(CallbackQuery::ProviderError {
                error: "access_denied".to_string(),
                error_description: Some("User cancelled".to_string()),
                state: Some("s".to_string()),
            })
        );
        // An error wins over a code
        assert_eq!(
            parse_query("error=server_error&code=c"),
            Some(CallbackQuery::ProviderError {
                error: "server_error".to_string(),
                error_description: None,
                state: None,
            })
        );
    }

    #[test]
    fn callback_query_without_code_or_state_is_rejected() {
        assert_eq!(parse_query(""), None);
        assert_eq!(parse_query("state=s"), None);
        assert_eq!(parse_query("code=c"), None);
    }

    /// The provider redirects to the callback with `error` instead of `code`
    /// when the user cancels the login.
    #[tokio::test]
    async fn provider_error_redirects_to_login_page() {
        let jar = PrivateCookieJar::from_headers(&HeaderMap::new(), Key::from(TEST_KEY));

        let result = callback(
            jar,
            Query(parse_query("error=access_denied&state=abc").unwrap()),
            State(state_with_origin_path("/kellnr")),
            Extension(None),
        )
        .await;

        let Ok((_, redirect)) = result else {
            panic!("expected a redirect");
        };
        assert_eq!(location(redirect), "/kellnr/login?error=access_denied");
    }

    #[tokio::test]
    async fn error_callback_redirects_below_origin_path() {
        let settings = state_with_origin_path("/kellnr").settings;

        let redirect = error_callback(
            State(settings),
            Query(ErrorQuery {
                error: "access_denied".to_string(),
                error_description: Some("User cancelled".to_string()),
            }),
        )
        .await;

        assert_eq!(location(redirect), "/kellnr/login?error=User%20cancelled");
    }
}
