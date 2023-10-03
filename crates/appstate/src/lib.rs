use axum::extract::FromRef;
use axum_extra::extract::cookie::Key;
use db::DbProvider;
use settings::Settings;
use std::sync::Arc;

pub type AppState = axum::extract::State<Arc<AppStateData>>;
pub type ArcAppState = Arc<AppStateData>;

pub struct AppStateData {
    pub db: Box<dyn DbProvider>,
    // key that is used for signing cookies
    pub signing_key: Key,
    pub settings: Settings,
}

impl FromRef<AppStateData> for Key {
    fn from_ref(state: &AppStateData) -> Self {
        state.signing_key.clone()
    }
}
