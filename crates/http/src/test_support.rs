use std::borrow::Cow;
use std::sync::Arc;

use axum_extra::extract::cookie::Key;
use cookie::{Cookie, CookieJar};
use kellnr_common::account_name::UserName;
use kellnr_common::background_tasks::BackgroundTasks;
use kellnr_cratesio_sync::CratesioSync;
use kellnr_db::DbProvider;
use kellnr_db::download_counter::DownloadCounter;
use kellnr_storage::cached_crate_storage::DynStorage;
use kellnr_storage::cratesio_crate_storage::CratesIoCrateStorage;
use kellnr_storage::docs_storage::DocsStorage;
use kellnr_storage::fs_storage::FSStorage;
use kellnr_storage::kellnr_crate_storage::KellnrCrateStorage;

use crate::identity::{Identity, Source};
use crate::login_throttle::LoginThrottle;
use crate::state::{AppStateData, default_settings_prov};
use crate::token_cache::TokenCacheManager;

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
    let proxy_client = kellnr_cratesio_sync::cratesio_downloader::CLIENT.clone();
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
        background_tasks: BackgroundTasks::new(),
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

/// A user logged in with a session cookie.
pub fn session_identity(name: &str, is_admin: bool) -> Identity {
    Identity {
        name: UserName::new(name).unwrap(),
        is_admin,
        is_read_only: false,
        source: Source::Session,
    }
}
