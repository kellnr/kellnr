use std::sync::Arc;

use axum::extract::FromRef;
use axum_extra::extract::cookie::Key;
use flume::Sender;
use kellnr_common::cratesio_prefetch_msg::CratesioPrefetchMsg;
use kellnr_cratesio_sync::CratesioSync;
use kellnr_db::DbProvider;
use kellnr_db::download_counter::DownloadCounter;
use kellnr_filter::DownloadFilters;
use kellnr_settings::{Settings, SettingsProv};
use kellnr_storage::cratesio_crate_storage::CratesIoCrateStorage;
use kellnr_storage::docs_storage::DocsStorage;
use kellnr_storage::kellnr_crate_storage::KellnrCrateStorage;
use kellnr_storage::toolchain_storage::ToolchainStorage;
use reqwest::Client;

use crate::login_throttle::LoginThrottle;
use crate::token_cache::TokenCacheManager;

pub type AppState = axum::extract::State<AppStateData>;

// Substates
pub type DbState = axum::extract::State<Arc<dyn DbProvider>>;
pub type SettingsState = axum::extract::State<Arc<Settings>>;
pub type SettingsProvState = axum::extract::State<Arc<SettingsProv>>;
pub type CrateStorageState = axum::extract::State<Arc<KellnrCrateStorage>>;
pub type CrateIoStorageState = axum::extract::State<Arc<CratesIoCrateStorage>>;
pub type DocsStorageState = axum::extract::State<Arc<DocsStorage>>;
pub type SigningKeyState = axum::extract::State<Key>;
pub type CratesIoPrefetchSenderState = axum::extract::State<Sender<CratesioPrefetchMsg>>;
pub type TokenCacheState = axum::extract::State<Arc<TokenCacheManager>>;
pub type LoginThrottleState = axum::extract::State<Arc<LoginThrottle>>;
pub type ToolchainStorageState = axum::extract::State<Option<Arc<ToolchainStorage>>>;
pub type DownloadCounterState = axum::extract::State<Arc<DownloadCounter>>;
pub type ProxyClientState = axum::extract::State<Client>;

#[derive(Clone, FromRef)]
pub struct AppStateData {
    pub db: Arc<dyn DbProvider>,
    // key that is used for signing cookies
    pub signing_key: Key,
    pub settings: Arc<Settings>,
    /// Per-leaf provenance for the merged `settings`. Only the `/settings`
    /// web-ui handler reads this; everything else uses the plain `Settings`.
    pub settings_prov: Arc<SettingsProv>,
    pub crate_storage: Arc<KellnrCrateStorage>,
    pub cratesio_storage: Arc<CratesIoCrateStorage>,
    /// Always present, unlike `toolchain_storage`: `/docs/*` serving and
    /// manual doc publishing are mounted regardless of `settings.docs.enabled`
    /// (that flag only gates the automatic generation queue).
    pub docs_storage: Arc<DocsStorage>,
    pub cratesio_prefetch_sender: Sender<CratesioPrefetchMsg>,
    pub token_cache: Arc<TokenCacheManager>,
    /// Per-username throttle for password authentication attempts.
    pub login_throttle: Arc<LoginThrottle>,
    pub toolchain_storage: Option<Arc<ToolchainStorage>>,
    pub download_counter: Arc<DownloadCounter>,
    pub proxy_client: Client,
    pub cratesio_sync: CratesioSync,
    pub download_filters: Arc<DownloadFilters>,
}

/// Build a defaults-only `SettingsProv`, every leaf reports
/// `Category::Default`. Convenient for ad-hoc test `AppStateData`
/// construction where the `settings_prov` field isn't otherwise relevant.
#[must_use]
pub fn default_settings_prov() -> Arc<SettingsProv> {
    Arc::new(
        provcfg::Config::new()
            .build::<SettingsProv>()
            .expect("default SettingsProv build is infallible"),
    )
}
