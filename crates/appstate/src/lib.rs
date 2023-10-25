use axum::extract::FromRef;
use axum_extra::extract::cookie::Key;
use db::DbProvider;
use settings::Settings;
use storage::kellnr_crate_storage::KellnrCrateStorage;
use std::sync::Arc;
use flume::Sender;
use common::cratesio_prefetch_msg::CratesioPrefetchMsg;

pub type AppState = axum::extract::State<AppStateData>;

// Substates
pub type DbState = axum::extract::State<Arc<dyn DbProvider>>;
pub type SettingsState = axum::extract::State<Arc<Settings>>;
pub type CrateStorageState = axum::extract::State<Arc<KellnrCrateStorage>>;
pub type SigningKeyState = axum::extract::State<Key>;
pub type CratesIoPrefetchSenderState = axum::extract::State<Arc<Sender<CratesioPrefetchMsg>>>;

#[derive(Clone, FromRef)]
pub struct AppStateData {
    pub db: Arc<dyn DbProvider>,
    // key that is used for signing cookies
    pub signing_key: Key,
    pub settings: Arc<Settings>,
    pub crate_storage: Arc<KellnrCrateStorage>,
    pub cratesio_prefetch_sender: Arc<Sender<CratesioPrefetchMsg>>,
}

#[cfg(test)]
mod test {
    use db::mock::MockDb;

    pub use super::*;

    pub async fn test_state() -> AppStateData {
        let db = Arc::new(MockDb::new());
        let signing_key = Key::generate();
        let settings = Arc::new(Settings::new().unwrap());
        let crate_storage = Arc::new(KellnrCrateStorage::new(&settings).await.unwrap());
        let (cratesio_prefetch_sender, _) = flume::unbounded();
        AppStateData {
            db,
            signing_key,
            settings,
            crate_storage,
            cratesio_prefetch_sender: Arc::new(cratesio_prefetch_sender),
        }
    }
}
