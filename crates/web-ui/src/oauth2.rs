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
use kellnr_appstate::{AppState, AppStateData, DbState, SettingsState};
use kellnr_auth::oauth2::{OAuth2Handler, UserInfo, generate_unique_username};
use kellnr_db::User;
use kellnr_settings::constants::COOKIE_OIDC_ID_TOKEN;
use serde::{Deserialize, Serialize};
use tracing::{error, trace, warn};
use utoipa::ToSchema;

use crate::error::RouteError;
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

/// Query parameters received in the `OAuth2` callback
#[derive(Debug, Deserialize, ToSchema, utoipa::IntoParams)]
pub struct CallbackQuery {
    /// Authorization code from the provider
    pub code: String,
    /// CSRF protection state
    pub state: String,
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
/// 2. Stores state/PKCE/nonce in the database
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
    State(db): DbState,
    Extension(oauth2_handler): OAuth2Ext,
) -> Result<Redirect, RouteError> {
    trace!("OAuth2 login initiated");

    // Check if OAuth2 is enabled
    let handler = oauth2_handler.as_ref().ok_or_else(|| {
        warn!("OAuth2 login attempted but OAuth2 is not enabled");
        RouteError::Status(StatusCode::NOT_FOUND)
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
        RouteError::Status(StatusCode::INTERNAL_SERVER_ERROR)
    })?;

    // Redirect to provider
    Ok(Redirect::to(auth_request.auth_url.as_str()))
}

/// Handle `OAuth2` callback from the provider
///
/// This endpoint:
/// 1. Validates the CSRF state
/// 2. Exchanges the authorization code for tokens
/// 3. Validates the ID token
/// 4. Creates or links the user account
/// 5. Creates a session and sets the session cookie
/// 6. Redirects to the UI
#[utoipa::path(
    get,
    path = "/callback",
    tag = "oauth2",
    params(CallbackQuery),
    responses(
        (status = 302, description = "Redirect to UI after successful login"),
        (status = 400, description = "Invalid state"),
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
) -> Result<(PrivateCookieJar, Redirect), RouteError> {
    trace!(state = %query.state, "OAuth2 callback received");

    // Check if OAuth2 is enabled
    let handler = oauth2_handler.as_ref().ok_or_else(|| {
        warn!("OAuth2 callback received but OAuth2 is not enabled");
        RouteError::Status(StatusCode::NOT_FOUND)
    })?;

    // Retrieve and validate state from database
    let state_data = app_state
        .db
        .get_and_delete_oauth2_state(&query.state)
        .await
        .map_err(|e| {
            error!("Failed to retrieve OAuth2 state: {}", e);
            RouteError::Status(StatusCode::INTERNAL_SERVER_ERROR)
        })?
        .ok_or_else(|| {
            warn!(
                "OAuth2 callback with invalid or expired state: {}",
                query.state
            );
            RouteError::Status(StatusCode::BAD_REQUEST)
        })?;

    // Exchange code for tokens and validate
    let token_result = handler
        .exchange_and_validate(&query.code, &state_data.pkce_verifier, &state_data.nonce)
        .await
        .map_err(|e| {
            error!("Failed to exchange OAuth2 code: {}", e);
            RouteError::Status(StatusCode::UNAUTHORIZED)
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
            RouteError::Status(StatusCode::INTERNAL_SERVER_ERROR)
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
                return Err(RouteError::Status(StatusCode::FORBIDDEN));
            }

            // Generate unique username
            let username = generate_unique_username(&user_info, |name| {
                let db = app_state.db.clone();
                async move { db.is_username_available(&name).await.unwrap_or(false) }
            })
            .await;

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
                    user_info.is_admin,
                    user_info.is_read_only,
                )
                .await
                .map_err(|e| {
                    error!("Failed to create OAuth2 user: {}", e);
                    RouteError::Status(StatusCode::INTERNAL_SERVER_ERROR)
                })?
        }
    };

    let jar = create_session_jar(cookies, &app_state, &user.name).await?;
    let jar = jar.add(session_cookie(
        COOKIE_OIDC_ID_TOKEN,
        token_result.id_token.clone(),
        app_state.settings.registry.session_age_seconds as i64,
    ));
    trace!("Created session for OAuth2 user: {}", user.name);

    // Redirect to UI root
    let mut base_path = app_state.settings.origin.path.clone();
    if !base_path.ends_with('/') {
        base_path.push('/');
    }
    if !base_path.starts_with('/') {
        base_path.insert(0, '/');
    }
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
) -> Result<User, RouteError> {
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
                RouteError::Status(StatusCode::INTERNAL_SERVER_ERROR)
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
                RouteError::Status(StatusCode::INTERNAL_SERVER_ERROR)
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

/// Error callback for `OAuth2` flow
///
/// Handles errors from the `OAuth2` provider (e.g., user cancelled, access denied)
#[derive(Debug, Deserialize, ToSchema, utoipa::IntoParams)]
pub struct ErrorQuery {
    pub error: String,
    #[serde(default)]
    pub error_description: Option<String>,
}

/// Handle `OAuth2` error callback
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
pub async fn error_callback(Query(query): Query<ErrorQuery>) -> Redirect {
    warn!(
        "OAuth2 error callback: {} - {:?}",
        query.error, query.error_description
    );

    // Redirect to login page with error parameter
    let error_msg = query.error_description.as_deref().unwrap_or(&query.error);
    let encoded_error = urlencoding::encode(error_msg);
    Redirect::to(&format!("/login?error={encoded_error}"))
}

#[cfg(test)]
mod sync_tests {
    use std::sync::Arc;

    use kellnr_db::mock::MockDb;
    use mockall::predicate::eq;

    use super::*;

    fn user(is_admin: bool, is_read_only: bool) -> User {
        User {
            id: 1,
            name: "alice".to_string(),
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
            ..kellnr_appstate::test_state()
        }
    }

    #[tokio::test]
    async fn read_only_governed_promotes_user() {
        let mut db = MockDb::new();
        db.expect_change_read_only_state()
            .with(eq("alice"), eq(true))
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
            .with(eq("alice"), eq(false))
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
