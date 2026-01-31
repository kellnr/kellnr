//! OAuth2/OIDC route configuration
//!
//! These routes handle OAuth2/OpenID Connect authentication flow.

use kellnr_appstate::AppStateData;
use kellnr_web_ui::oauth2;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

/// Create `OAuth2` routes
///
/// Routes:
/// - GET /config - Returns `OAuth2` configuration for the UI
/// - GET /login - Initiates `OAuth2` flow, redirects to provider
/// - GET /callback - Handles callback from provider
/// - GET /error - Handles error responses from provider
pub fn create_routes() -> OpenApiRouter<AppStateData> {
    OpenApiRouter::new()
        .routes(routes!(oauth2::get_config))
        .routes(routes!(oauth2::login))
        .routes(routes!(oauth2::callback))
        .routes(routes!(oauth2::error_callback))
}
