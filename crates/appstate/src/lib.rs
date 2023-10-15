use axum::extract::FromRef;
use axum_extra::extract::cookie::Key;
use db::DbProvider;
use settings::Settings;
use std::sync::Arc;
use registry::kellnr_crate_storage::KellnrCrateStorage;

pub type AppState = axum::extract::State<AppStateData>;

// Substates
pub type DbState = axum::extract::State<Arc<dyn DbProvider>>;
pub type SettingsState = axum::extract::State<Arc<Settings>>;

#[derive(Clone, FromRef)]
pub struct AppStateData {
    pub db: Arc<dyn DbProvider>,
    // key that is used for signing cookies
    pub signing_key: Key,
    pub settings: Arc<Settings>,
    pub crate_storage: Arc<KellnrCrateStorage>,
}
