use std::sync::Arc;

use axum::extract::FromRef;
use axum_extra::extract::cookie::Key;
use flume::Sender;
use kellnr_common::cratesio_prefetch_msg::CratesioPrefetchMsg;
use kellnr_common::token_cache::TokenCacheManager;
use kellnr_db::DbProvider;
use reqwest::Client;
use kellnr_db::download_counter::DownloadCounter;
use kellnr_settings::Settings;
use kellnr_storage::cached_crate_storage::DynStorage;
use kellnr_storage::cratesio_crate_storage::CratesIoCrateStorage;
use kellnr_storage::fs_storage::FSStorage;
use kellnr_storage::kellnr_crate_storage::KellnrCrateStorage;
use kellnr_storage::toolchain_storage::ToolchainStorage;

pub type AppState = axum::extract::State<AppStateData>;

// Substates
pub type DbState = axum::extract::State<Arc<dyn DbProvider>>;
pub type SettingsState = axum::extract::State<Arc<Settings>>;
pub type CrateStorageState = axum::extract::State<Arc<KellnrCrateStorage>>;
pub type CrateIoStorageState = axum::extract::State<Arc<CratesIoCrateStorage>>;
pub type SigningKeyState = axum::extract::State<Key>;
pub type CratesIoPrefetchSenderState = axum::extract::State<Sender<CratesioPrefetchMsg>>;
pub type TokenCacheState = axum::extract::State<Arc<TokenCacheManager>>;
pub type ToolchainStorageState = axum::extract::State<Option<Arc<ToolchainStorage>>>;
pub type DownloadCounterState = axum::extract::State<Arc<DownloadCounter>>;
pub type ProxyClientState = axum::extract::State<Client>;

#[derive(Clone, FromRef)]
pub struct AppStateData {
    pub db: Arc<dyn DbProvider>,
    // key that is used for signing cookies
    pub signing_key: Key,
    pub settings: Arc<Settings>,
    pub crate_storage: Arc<KellnrCrateStorage>,
    pub cratesio_storage: Arc<CratesIoCrateStorage>,
    pub cratesio_prefetch_sender: Sender<CratesioPrefetchMsg>,
    pub token_cache: Arc<TokenCacheManager>,
    pub toolchain_storage: Option<Arc<ToolchainStorage>>,
    pub download_counter: Arc<DownloadCounter>,
    pub proxy_client: Client,
}

pub fn test_state() -> AppStateData {
    let db: Arc<dyn DbProvider> = Arc::new(kellnr_db::mock::MockDb::new());
    let signing_key = Key::generate();
    let settings = Arc::new(kellnr_settings::test_settings());
    let kellnr_storage = Box::new(FSStorage::new(&settings.crates_path()).unwrap()) as DynStorage;
    let crate_storage = Arc::new(KellnrCrateStorage::new(&settings, kellnr_storage));
    let cratesio_storage = Arc::new(CratesIoCrateStorage::new(
        &settings,
        Box::new(FSStorage::new(&settings.crates_io_path()).unwrap()) as DynStorage,
    ));
    let (cratesio_prefetch_sender, _) = flume::unbounded();
    let token_cache = Arc::new(TokenCacheManager::new(false, 60, 1000));
    let download_counter = Arc::new(DownloadCounter::new(db.clone(), 30));
    AppStateData {
        db,
        signing_key,
        settings,
        crate_storage,
        cratesio_storage,
        cratesio_prefetch_sender,
        token_cache,
        toolchain_storage: None, // Toolchain storage disabled in tests by default
        download_counter,
        proxy_client: kellnr_common::cratesio_downloader::CLIENT.clone(),
    }
}
