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
use axum_extra::extract::cookie::Cookie;
use cookie::time;
use kellnr_appstate::{AppState, DbState, SettingsState};
use kellnr_auth::oauth2::{OAuth2Handler, generate_unique_username};
use kellnr_common::util::generate_rand_string;
use kellnr_settings::constants::COOKIE_SESSION_ID;
use serde::{Deserialize, Serialize};
use tracing::{error, info, trace, warn};
use utoipa::ToSchema;

use crate::error::RouteError;

/// Type alias for the `OAuth2` handler extension
pub type OAuth2Ext = Extension<Option<Arc<OAuth2Handler>>>;

/// `OAuth2` configuration response for the UI
#[derive(Debug, Serialize, ToSchema)]
pub struct OAuth2Config {
    /// Whether `OAuth2` authentication is enabled
    pub enabled: bool,
    /// Text to display on the login button
    pub button_text: String,
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

    info!(
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
            info!("Found existing user '{}' for OAuth2 identity", user.name);
            user
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

            info!(
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

    // Create session
    let session_token = generate_rand_string(12);
    app_state
        .db
        .add_session_token(&user.name, &session_token)
        .await
        .map_err(|e| {
            error!("Failed to create session: {}", e);
            RouteError::Status(StatusCode::INTERNAL_SERVER_ERROR)
        })?;

    info!("Created session for OAuth2 user: {}", user.name);

    // Set session cookie
    let jar = cookies.add(
        Cookie::build((COOKIE_SESSION_ID, session_token))
            .max_age(time::Duration::seconds(
                app_state.settings.registry.session_age_seconds as i64,
            ))
            .same_site(axum_extra::extract::cookie::SameSite::Strict)
            .path("/"),
    );

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
