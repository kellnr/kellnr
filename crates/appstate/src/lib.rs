use axum::extract::FromRef;
use axum_extra::extract::cookie::Key;
use common::cratesio_prefetch_msg::CratesioPrefetchMsg;
use db::DbProvider;
use flume::Sender;
use settings::Settings;
use std::sync::Arc;
use storage::{
    cratesio_crate_storage::CratesIoCrateStorage, kellnr_crate_storage::KellnrCrateStorage,
};

pub type AppState = axum::extract::State<AppStateData>;

// Substates
pub type DbState = axum::extract::State<Arc<dyn DbProvider>>;
pub type SettingsState = axum::extract::State<Arc<Settings>>;
pub type CrateStorageState = axum::extract::State<Arc<KellnrCrateStorage>>;
pub type CrateIoStorageState = axum::extract::State<Arc<CratesIoCrateStorage>>;
pub type SigningKeyState = axum::extract::State<Key>;
pub type CratesIoPrefetchSenderState = axum::extract::State<Arc<Sender<CratesioPrefetchMsg>>>;

#[derive(Clone, FromRef)]
pub struct AppStateData {
    pub db: Arc<dyn DbProvider>,
    // key that is used for signing cookies
    pub signing_key: Key,
    pub settings: Arc<Settings>,
    pub crate_storage: Arc<KellnrCrateStorage>,
    pub cratesio_storage: Arc<CratesIoCrateStorage>,
    pub cratesio_prefetch_sender: Arc<Sender<CratesioPrefetchMsg>>,
}

pub async fn test_state() -> AppStateData {
    let db = Arc::new(db::mock::MockDb::new());
    let signing_key = Key::generate();
    let settings = Arc::new(Settings::default());
    let crate_storage = Arc::new(KellnrCrateStorage::new(&settings).await.unwrap());
    let crateio_storage = Arc::new(CratesIoCrateStorage::new(&settings).await.unwrap());
    let (cratesio_prefetch_sender, _) = flume::unbounded();
    AppStateData {
        db,
        signing_key,
        settings,
        crate_storage,
        cratesio_storage: crateio_storage,
        cratesio_prefetch_sender: Arc::new(cratesio_prefetch_sender),
    }
}
