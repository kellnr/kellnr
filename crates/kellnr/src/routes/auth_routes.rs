use kellnr_appstate::AppStateData;
use kellnr_web_ui::user;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

/// Creates the authentication routes
pub fn create_routes() -> OpenApiRouter<AppStateData> {
    OpenApiRouter::new()
        .routes(routes!(user::login))
        .routes(routes!(user::logout))
        .routes(routes!(user::login_state))
}
