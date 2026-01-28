//! OAuth2/OIDC route configuration
//!
//! These routes handle OAuth2/OpenID Connect authentication flow.

use axum::Router;
use axum::routing::get;
use kellnr_appstate::AppStateData;
use kellnr_web_ui::oauth2;

/// Create `OAuth2` routes
///
/// Routes:
/// - GET /config - Returns `OAuth2` configuration for the UI
/// - GET /login - Initiates `OAuth2` flow, redirects to provider
/// - GET /callback - Handles callback from provider
/// - GET /error - Handles error responses from provider
pub fn create_routes() -> Router<AppStateData> {
    Router::new()
        .route("/config", get(oauth2::get_config))
        .route("/login", get(oauth2::login))
        .route("/callback", get(oauth2::callback))
        .route("/error", get(oauth2::error_callback))
}
