use axum::Router;
use axum::routing::{get, post};
use kellnr_appstate::AppStateData;
use kellnr_web_ui::user;

/// Creates the authentication routes
pub fn create_routes() -> Router<AppStateData> {
    Router::new()
        .route("/login", post(user::login))
        .route("/logout", post(user::logout))
        .route("/state", get(user::login_state))
}
