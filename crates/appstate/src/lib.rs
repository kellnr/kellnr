use std::sync::Arc;
use db::DbProvider;

pub type AppState = axum::extract::State<Arc<AppStateData>>;

pub struct AppStateData {
    pub db: Box<dyn DbProvider>
}

