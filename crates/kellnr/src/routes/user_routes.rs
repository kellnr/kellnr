use kellnr_appstate::AppStateData;
use kellnr_web_ui::user;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

/// Creates the user management routes
pub fn create_routes() -> OpenApiRouter<AppStateData> {
    OpenApiRouter::new()
        // User CRUD
        .routes(routes!(user::list_users, user::add))
        .routes(routes!(user::delete))
        // User attributes
        .routes(routes!(user::reset_pwd))
        .routes(routes!(user::admin))
        .routes(routes!(user::read_only))
        // Current user (self-service)
        .routes(routes!(user::change_pwd))
        .routes(routes!(user::list_tokens, user::add_token))
        .routes(routes!(user::delete_token))
}
