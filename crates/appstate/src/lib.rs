use std::borrow::Cow;
use std::sync::Arc;

use axum::extract::FromRef;
use axum_extra::extract::cookie::Key;
use cookie::{Cookie, CookieJar};
use flume::Sender;
use kellnr_common::cratesio_prefetch_msg::CratesioPrefetchMsg;
use kellnr_common::login_throttle::LoginThrottle;
use kellnr_common::token_cache::TokenCacheManager;
use kellnr_cratesio_sync::CratesioSync;
use kellnr_db::DbProvider;
use kellnr_db::download_counter::DownloadCounter;
use kellnr_filter::DownloadFilters;
use kellnr_settings::{Settings, SettingsProv};
use kellnr_storage::cached_crate_storage::DynStorage;
use kellnr_storage::cratesio_crate_storage::CratesIoCrateStorage;
use kellnr_storage::docs_storage::DocsStorage;
use kellnr_storage::fs_storage::FSStorage;
use kellnr_storage::kellnr_crate_storage::KellnrCrateStorage;
use kellnr_storage::toolchain_storage::ToolchainStorage;
use reqwest::Client;

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

pub fn test_state() -> AppStateData {
    let db: Arc<dyn DbProvider> = Arc::new(kellnr_db::mock::MockDb::new());
    let signing_key = Key::generate();
    let settings = Arc::new(kellnr_settings::test_settings());
    let settings_prov = default_settings_prov();
    let kellnr_storage = Box::new(FSStorage::new(&settings.crates_path()).unwrap()) as DynStorage;
    let crate_storage = Arc::new(KellnrCrateStorage::new(&settings, kellnr_storage));
    let cratesio_storage = Arc::new(CratesIoCrateStorage::new(
        &settings,
        Box::new(FSStorage::new(&settings.crates_io_path()).unwrap()) as DynStorage,
    ));
    let docs_storage = Arc::new(DocsStorage::new(Box::new(
        FSStorage::new(&settings.docs_path()).unwrap(),
    ) as DynStorage));
    let (cratesio_prefetch_sender, _) = flume::unbounded();
    let token_cache = Arc::new(TokenCacheManager::new(
        false,
        std::time::Duration::from_secs(60),
        1000,
    ));
    let login_throttle = Arc::new(LoginThrottle::new(
        settings.registry.auth_max_failed_attempts,
        settings.registry.auth_failed_attempts_window(),
    ));
    let download_counter = Arc::new(DownloadCounter::new(db.clone(), 30));
    let proxy_client = kellnr_common::cratesio_downloader::CLIENT.clone();
    let cratesio_sync = CratesioSync::new(db.clone(), proxy_client.clone(), &settings.proxy);
    AppStateData {
        db,
        signing_key,
        settings,
        settings_prov,
        crate_storage,
        cratesio_storage,
        docs_storage,
        cratesio_prefetch_sender,
        token_cache,
        login_throttle,
        toolchain_storage: None, // Toolchain storage disabled in tests by default
        download_counter,
        proxy_client,
        cratesio_sync,
        download_filters: Arc::default(),
    }
}

/// Fixed cookie signing key for tests. Use it as `signing_key` in the app
/// state when requests carry cookies created by [`encode_cookies`].
pub const TEST_KEY: &[u8] = &[1; 64];

/// Encodes cookies as a `Cookie` header value, encrypted with [`TEST_KEY`] the
/// same way `PrivateCookieJar` does.
pub fn encode_cookies<const N: usize, K: Into<Cow<'static, str>>, V: Into<Cow<'static, str>>>(
    cookies: [(K, V); N],
) -> String {
    let mut clear = CookieJar::new();
    let mut jar = clear.private_mut(&Key::from(TEST_KEY));
    for (k, v) in cookies {
        jar.add(Cookie::new(k, v));
    }
    clear
        .iter()
        .map(|c| c.encoded().to_string())
        .collect::<Vec<_>>()
        .join("; ")
}
