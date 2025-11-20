use appstate::AppStateData;
use axum::{
    Router,
    routing::{delete, get, post},
};
use web_ui::user;

/// Creates the user routes
pub fn create_routes() -> Router<AppStateData> {
    Router::new()
        .route("/login", post(user::login))
        .route("/logout", get(user::logout))
        .route("/change_pwd", post(user::change_pwd))
        .route("/add", post(user::add))
        .route("/delete/{name}", delete(user::delete))
        .route("/reset_pwd/{name}", post(user::reset_pwd))
        .route("/read_only/{name}", post(user::read_only))
        .route("/add_token", post(user::add_token))
        .route("/delete_token/{id}", delete(user::delete_token))
        .route("/list_tokens", get(user::list_tokens))
        .route("/list_users", get(user::list_users))
        .route("/login_state", get(user::login_state))
}
