use db::DbProvider;
use settings::Settings;
use std::sync::Arc;

pub type AppState = axum::extract::State<Arc<AppStateData>>;
pub type ArcAppState = Arc<AppStateData>;

pub struct AppStateData {
    pub db: Box<dyn DbProvider>,
    pub settings: Settings,
}
