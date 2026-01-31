use axum::Router;
use axum::routing::{delete, get, post, put};
use kellnr_appstate::AppStateData;
use kellnr_web_ui::user;

/// Creates the user management routes
pub fn create_routes() -> Router<AppStateData> {
    Router::new()
        // User CRUD
        .route("/", get(user::list_users))
        .route("/", post(user::add))
        .route("/{name}", delete(user::delete))
        // User attributes
        .route("/{name}/password", put(user::reset_pwd))
        .route("/{name}/admin", post(user::admin))
        .route("/{name}/read-only", post(user::read_only))
        // Current user (self-service)
        .route("/me/password", put(user::change_pwd))
        .route("/me/tokens", get(user::list_tokens))
        .route("/me/tokens", post(user::add_token))
        .route("/me/tokens/{id}", delete(user::delete_token))
}
