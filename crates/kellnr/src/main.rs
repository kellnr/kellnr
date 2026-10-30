use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use axum::response::Redirect;
use axum::routing::get;
use axum_extra::extract::cookie::Key;
use kellnr_appstate::AppStateData;
use kellnr_auth::oauth2::{OAuth2Error, OAuth2Handler};
use kellnr_common::cratesio_downloader::build_client;
use kellnr_common::cratesio_prefetch_msg::CratesioPrefetchMsg;
use kellnr_common::login_throttle::LoginThrottle;
use kellnr_common::token_cache::TokenCacheManager;
use kellnr_cratesio_sync::CratesioSync;
use kellnr_db::download_counter::DownloadCounter;
use kellnr_db::{ConString, Database, DbProvider, PgConString, SqliteConString};
use kellnr_filter::DownloadFilters;
use kellnr_index::cratesio_prefetch_api::{
    CratesIoPrefetchArgs, UPDATE_CACHE_TIMEOUT_SECS, init_cratesio_prefetch_thread,
};
use kellnr_settings::{
    CliResult, LogFormat, ResolvedSettings, Settings, ShowConfigOptions, parse_cli,
};
use kellnr_storage::cached_crate_storage::DynStorage;
use kellnr_storage::cratesio_crate_storage::CratesIoCrateStorage;
use kellnr_storage::docs_storage::DocsStorage;
use kellnr_storage::fs_storage::FSStorage;
use kellnr_storage::gcs_storage::GCSStorage;
use kellnr_storage::kellnr_crate_storage::KellnrCrateStorage;
use kellnr_storage::s3_storage::S3Storage;
use kellnr_storage::toolchain_storage::ToolchainStorage;
use moka::future::Cache;
use tokio::fs::create_dir_all;
use tokio::net::TcpListener;
use tracing::{error, info, trace, warn};
use tracing_subscriber::fmt::format;

mod config_printer;
mod openapi;
mod routes;

#[tokio::main]
async fn main() {
    let cli_result = parse_cli().expect("Cannot read config");

    match cli_result {
        CliResult::ShowConfig { resolved, options } => {
            show_config(&resolved, &options);
        }
        CliResult::InitConfig { output } => {
            init_config(&output);
        }
        CliResult::RunServer(resolved) => {
            run_server(resolved).await;
        }
        CliResult::ShowHelp => {
            // Help was already printed by parse_cli()
        }
        CliResult::BuildDocs {
            crate_path,
            cratesio_index,
        } => {
            if let Err(e) =
                kellnr_docs::doc_queue::generate_docs(&crate_path, cratesio_index.as_deref())
            {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
    }
}

fn show_config(resolved: &ResolvedSettings, options: &ShowConfigOptions) {
    if options.no_defaults || options.show_sources {
        // Use custom formatting with source annotations and/or filtered output
        config_printer::print_config_with_options(&resolved.prov, options);
    } else {
        // Default: simple TOML output
        match toml::to_string_pretty(&resolved.settings) {
            Ok(toml) => println!("{toml}"),
            Err(e) => {
                eprintln!("Error serializing config: {e}");
                std::process::exit(1);
            }
        }
    }
}

fn init_config(output: &Path) {
    if output.exists() {
        eprintln!("Error: File already exists: {}", output.display());
        eprintln!("Remove the file or specify a different output path with -o");
        std::process::exit(1);
    }

    match toml::to_string_pretty(&Settings::default()) {
        Ok(toml) => match std::fs::write(output, toml) {
            Ok(()) => {
                println!("Configuration file created: {}", output.display());
            }
            Err(e) => {
                eprintln!("Error writing config file: {e}");
                std::process::exit(1);
            }
        },
        Err(e) => {
            eprintln!("Error serializing config: {e}");
            std::process::exit(1);
        }
    }
}

async fn run_server(resolved: ResolvedSettings) {
    let settings: Arc<Settings> = resolved.settings.into();
    let settings_prov = resolved.prov;

    // Validate required settings
    if settings.registry.data_dir.is_empty() {
        eprintln!("Error: No data directory configured.");
        eprintln!();
        eprintln!("Please set the data directory using one of the following methods:");
        eprintln!("  1. CLI argument:    kellnr start --registry-data-dir /path/to/data");
        eprintln!("  2. Environment var: KELLNR_REGISTRY__DATA_DIR=/path/to/data");
        eprintln!("  3. Config file:     registry.data_dir = \"/path/to/data\"");
        eprintln!();
        eprintln!("For more information, run: kellnr start --help");
        std::process::exit(1);
    }

    // Only one remote storage backend can be active at a time. Silently preferring one
    // would put crates in a different place than the operator configured.
    if settings.s3.enabled && settings.gcs.enabled {
        eprintln!("Error: Both the S3 and the GCS storage backend are enabled.");
        eprintln!();
        eprintln!("Kellnr can only use one storage backend at a time.");
        eprintln!("Disable one of them, for example:");
        eprintln!("  KELLNR_S3__ENABLED=false or KELLNR_GCS__ENABLED=false");
        std::process::exit(1);
    }

    if let Err(e) = settings.oauth2.validate() {
        eprintln!("Error: invalid OAuth2 configuration: {e}");
        std::process::exit(1);
    }

    let download_filters = Arc::new(init_download_filters(&settings));

    let addr = SocketAddr::from((settings.local.ip, settings.local.port));

    // Configure tracing subscriber
    init_tracing(&settings);

    info!("Starting kellnr");

    // Ensure the data directory exists, if not create it
    create_dir_all(&settings.registry.data_dir)
        .await
        .expect("Failed to create data directory.");

    // Initialize kellnr crate storage
    let crate_storage: Arc<KellnrCrateStorage> = init_kellnr_crate_storage(&settings).into();

    // Create the database connection. Has to be done after the index and storage
    // as the needed folders for the sqlite database may not have been created before that.
    let con_string = get_connect_string(&settings);
    let db = Database::new(&con_string, settings.registry.max_db_connections)
        .await
        .expect("Failed to create database");
    let db = Arc::new(db) as Arc<dyn DbProvider>;

    // Crates.io Proxy
    let cratesio_storage: Arc<CratesIoCrateStorage> = init_cratesio_storage(&settings).into();
    let (cratesio_prefetch_sender, cratesio_prefetch_receiver) =
        flume::unbounded::<CratesioPrefetchMsg>();

    let proxy_client = build_client(
        &settings.proxy.user_agent,
        Duration::from_secs(settings.proxy.connect_timeout_seconds),
        Duration::from_secs(settings.proxy.request_timeout_seconds),
    );
    let cratesio_sync = CratesioSync::new(db.clone(), proxy_client.clone(), &settings.proxy);

    let prefetch_args = CratesIoPrefetchArgs {
        db: db.clone(),
        cache: Cache::builder()
            .time_to_live(Duration::from_secs(UPDATE_CACHE_TIMEOUT_SECS))
            .build(),
        recv: cratesio_prefetch_receiver.clone(),
        download_on_update: settings.proxy.download_on_update,
        storage: cratesio_storage.clone(),
        url: settings.proxy.url.clone(),
        proxy_settings: Arc::new(settings.proxy.clone()),
        sync: cratesio_sync.clone(),
        filters: download_filters.cratesio.clone(),
    };

    init_cratesio_prefetch_thread(
        cratesio_prefetch_sender.clone(),
        settings.proxy.num_threads,
        prefetch_args,
    );

    // Docs hosting
    let docs_storage: Arc<DocsStorage> = init_docs_storage(&settings).into();
    init_docs_hosting(
        &settings,
        crate_storage.clone(),
        docs_storage.clone(),
        db.clone(),
    );

    // Webhook support
    init_webhook_service(&settings, db.clone());

    let signing_key = init_cookie_signing_key(&settings);
    let max_docs_size = settings.docs.max_size;
    let max_crate_size = settings.registry.max_crate_size as usize;
    let route_path_prefix = settings.origin.path.clone();
    let max_toolchain_size = settings.toolchain.max_size;
    let token_cache = Arc::new(TokenCacheManager::new(
        settings.registry.token_cache_enabled,
        settings.registry.token_cache_ttl_seconds,
        settings.registry.token_cache_max_capacity,
    ));
    let login_throttle = Arc::new(LoginThrottle::new(
        settings.registry.auth_max_failed_attempts,
        Duration::from_secs(settings.registry.auth_failed_attempts_window_seconds),
    ));

    // Initialize toolchain storage if enabled
    let toolchain_storage = init_toolchain_storage(&settings);

    // Initialize OAuth2/OIDC handler if enabled
    let oauth2_handler = init_oauth2_handler(&settings).await.unwrap_or_else(|e| {
        error!("Failed to initialize OAuth2/OIDC handler: {}", e);
        // With enforcement on, password login is rejected, so a handler that
        // failed to initialize for *any* reason leaves no way to log in at all.
        // Refuse to start instead of serving an unusable registry.
        if settings.oauth2.enforced {
            error!(
                "OAuth2 enforcement is enabled, so login is impossible without a working OIDC provider"
            );
            std::process::exit(1);
        }
        warn!("OAuth2/OIDC authentication will be disabled");
        None
    });

    // With enforcement on, the local admin can no longer log in, so an
    // administrator has to come from the IdP. That needs either an admin group
    // mapping, which promotes SSO users, or an admin who already linked an
    // OAuth2 identity. Warn when neither holds, rather than fail: the registry
    // itself still works, only admin access is unreachable.
    if settings.oauth2.enforced
        && (settings.oauth2.admin_group_claim.is_none()
            || settings.oauth2.admin_group_value.is_none())
        && !db.has_oauth2_admin().await.unwrap_or(false)
    {
        warn!(
            "OAuth2 enforcement is enabled without oauth2.admin_group_claim and \
             oauth2.admin_group_value, and no admin has an OAuth2 identity linked. \
             Users provisioned via SSO will never gain admin rights, and the local \
             admin account can no longer log in."
        );
    }

    // Initialize download counter with periodic flush
    let flush_interval = settings.registry.download_counter_flush_seconds;
    let download_counter = Arc::new(DownloadCounter::new(db.clone(), flush_interval));
    if flush_interval > 0 {
        let counter = download_counter.clone();
        trace!("Starting download counter flush task (interval: {flush_interval}s)");
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(flush_interval));
            // First tick completes immediately, skip it
            interval.tick().await;
            loop {
                interval.tick().await;
                counter.flush().await;
            }
        });
    }

    // Periodically remove sessions that have outlived `session_age_seconds`.
    // validate_session already rejects expired sessions on read; this keeps the
    // session table from growing unbounded. A configured age of zero disables
    // expiry, so the cleanup task is not started. Otherwise run at most hourly,
    // at least every minute, and once immediately on startup to clear sessions
    // left by a previous run.
    let session_age_seconds = settings.registry.session_age_seconds;
    if session_age_seconds > 0 {
        let session_cleanup_interval = session_age_seconds.clamp(60, 3600);
        let session_cleanup_db = db.clone();
        trace!("Starting session cleanup task (interval: {session_cleanup_interval}s)");
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(session_cleanup_interval));
            loop {
                interval.tick().await;
                match session_cleanup_db.delete_expired_sessions().await {
                    Ok(n) if n > 0 => trace!("Removed {n} expired session(s)"),
                    Ok(_) => {}
                    Err(e) => warn!("Failed to delete expired sessions: {e}"),
                }
            }
        });
    }

    // OAuth2 login attempts that were never completed leave their state rows
    // behind. They expire after ten minutes; remove them periodically so the
    // table cannot grow without bound.
    // The task also runs with OAuth2 disabled to clear rows left behind while
    // it was enabled.
    let oauth2_cleanup_db = db.clone();
    trace!("Starting OAuth2 state cleanup task");
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_mins(5));
        loop {
            interval.tick().await;
            match oauth2_cleanup_db.cleanup_expired_oauth2_states().await {
                Ok(n) if n > 0 => trace!("Removed {n} expired OAuth2 state(s)"),
                Ok(_) => {}
                Err(e) => warn!("Failed to delete expired OAuth2 states: {e}"),
            }
        }
    });

    let download_counter_for_shutdown = download_counter.clone();
    let sponsor_links_enabled = settings.registry.sponsor_links_enabled;

    let state = AppStateData {
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
        toolchain_storage,
        download_counter,
        proxy_client,
        cratesio_sync,
        download_filters,
    };

    // Create router using the route module
    let mut app = routes::create_router(
        state,
        max_docs_size,
        max_crate_size,
        max_toolchain_size,
        oauth2_handler,
    );
    if !route_path_prefix.is_empty() {
        let route_path_prefix_with_trailing = format!("{route_path_prefix}/");
        app = axum::Router::new()
            .nest(&route_path_prefix_with_trailing, app)
            .route(
                &route_path_prefix,
                get(move || async move {
                    Redirect::permanent(route_path_prefix_with_trailing.as_str())
                }),
            );
    }

    // Start the server with graceful shutdown
    let listener = TcpListener::bind(addr)
        .await
        .unwrap_or_else(|_| panic!("Failed to bind to {addr}"));
    info!("Kellnr has been started on http://{addr}/");
    if sponsor_links_enabled {
        info!(
            "Kellnr is maintained by one person in their spare time. If it saves your team time, consider sponsoring it: https://kellnr.io/sponsor"
        );
    }

    #[cfg(unix)]
    let sigterm = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let sigterm = std::future::pending::<()>();

    tokio::select! {
        result = axum::serve(listener, app) => {
            if let Err(e) = result {
                error!("Server error: {e}");
            }
        }
        _ = tokio::signal::ctrl_c() => {
            trace!("Shutdown signal received, flushing download counters...");
        }
        () = sigterm => {
            trace!("SIGTERM received, flushing download counters...");
        }
    }

    // Flush any accumulated download counts before exiting
    download_counter_for_shutdown.flush().await;
    trace!("Download counters flushed. Shutting down.");
}

fn init_cookie_signing_key(settings: &Settings) -> Key {
    // Either take the provided signing key or generate a random one.
    // A provided key can be shared between multiple instances of kellnr to
    // allow UI authentication in multi-instance scenarios.
    //
    // The key bytes are used as is. Deriving the key differently would
    // invalidate all existing sessions without adding entropy, so a weak key
    // is only reported.
    if let Some(key) = &settings.registry.cookie_signing_key {
        if is_low_variety_key(key) {
            warn!(
                "registry.cookie_signing_key consists of very few distinct characters and is \
                 easy to guess. Generate a random key, e.g. with `openssl rand -hex 64`."
            );
        }
        Key::try_from(key.as_bytes()).expect(
            "Failed to create cookie signing key from provided settings. The key has to be at \
             least 64 bytes long. Generate one, e.g. with `openssl rand -hex 64`.",
        )
    } else {
        Key::generate()
    }
}

/// Minimum number of distinct characters in a configured cookie signing key.
/// A hex encoded random key has 16; keys like `aaaa...` or `abcabc...` have
/// far fewer.
const MIN_DISTINCT_KEY_CHARS: usize = 8;

fn is_low_variety_key(key: &str) -> bool {
    key.bytes().collect::<std::collections::HashSet<_>>().len() < MIN_DISTINCT_KEY_CHARS
}

fn init_tracing(settings: &Settings) {
    let ts = tracing_subscriber::fmt()
        .with_max_level(settings.log.level)
        .with_env_filter(format!(
            "{},mio::poll=error,want=error,sqlx::query=error,sqlx::postgres=warn,\
                sea_orm_migration=warn,cargo=error,globset=warn,\
                hyper=warn,_=warn,reqwest=warn,tower_http={},\
                object_store::aws::builder=error,h2=error",
            settings.log.level, settings.log.level_web_server
        ));

    match settings.log.format {
        LogFormat::Compact => ts.event_format(format().compact()).init(),
        LogFormat::Pretty => ts.event_format(format().pretty()).init(),
        LogFormat::Json => ts.event_format(format().json()).init(),
    }
}

fn get_connect_string(settings: &Settings) -> ConString {
    if settings.postgresql.enabled {
        ConString::Postgres(PgConString::from(settings))
    } else {
        ConString::Sqlite(SqliteConString::from(settings))
    }
}

fn init_docs_hosting(
    settings: &Settings,
    cs: Arc<KellnrCrateStorage>,
    docs_storage: Arc<DocsStorage>,
    db: Arc<dyn DbProvider + 'static>,
) {
    if settings.docs.enabled {
        kellnr_docs::doc_queue::doc_extraction_queue(db, cs, docs_storage, settings);
    }
}

fn init_docs_storage(settings: &Settings) -> DocsStorage {
    let storage: DynStorage = if settings.s3.enabled
        && let Some(bucket) = &settings.s3.docs_bucket
    {
        Box::new(
            S3Storage::try_from((bucket.as_str(), settings)).expect("Failed to create S3 storage."),
        )
    } else if settings.gcs.enabled
        && let Some(bucket) = &settings.gcs.docs_bucket
    {
        Box::new(
            GCSStorage::try_from((bucket.as_str(), settings))
                .expect("Failed to create GCS storage."),
        )
    } else {
        Box::new(FSStorage::new(&settings.docs_path()).expect("Failed to create FS storage."))
    };
    DocsStorage::new(storage)
}

fn init_download_filters(settings: &Settings) -> DownloadFilters {
    DownloadFilters::from_settings(settings).unwrap_or_else(|e| {
        eprintln!("Error: {e}");
        std::process::exit(1);
    })
}

fn init_cratesio_storage(settings: &Settings) -> CratesIoCrateStorage {
    let storage = init_storage(&settings.crates_io_path_or_bucket(), settings);
    CratesIoCrateStorage::new(settings, storage)
}

fn init_kellnr_crate_storage(settings: &Settings) -> KellnrCrateStorage {
    let storage = init_storage(&settings.crates_path_or_bucket(), settings);
    KellnrCrateStorage::new(settings, storage)
}

fn init_storage(folder: &str, settings: &Settings) -> DynStorage {
    kellnr_storage::init_storage(folder, settings).expect("Failed to create storage.")
}

fn init_webhook_service(settings: &Settings, db: Arc<dyn DbProvider + 'static>) {
    kellnr_webhooks::run_webhook_service(db, &settings.webhooks);
}

fn init_toolchain_storage(settings: &Arc<Settings>) -> Option<Arc<ToolchainStorage>> {
    if !settings.toolchain.enabled {
        return None;
    }

    let storage = init_storage(&settings.toolchain_path_or_bucket(), settings);
    let toolchain_storage = ToolchainStorage::new(storage);

    Some(Arc::new(toolchain_storage))
}

async fn init_oauth2_handler(
    settings: &Settings,
) -> Result<Option<Arc<OAuth2Handler>>, OAuth2Error> {
    if !settings.oauth2.enabled {
        return Ok(None);
    }

    let callback_url = format!("{}/api/v1/oauth2/callback", settings.origin.base_url());

    OAuth2Handler::from_discovery(&settings.oauth2, &callback_url)
        .await
        .map(|handler| Some(Arc::new(handler)))
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use kellnr_settings::test_settings;

    use super::*;

    #[test]
    fn low_variety_cookie_keys_are_detected() {
        assert!(is_low_variety_key(&"a".repeat(64)));
        assert!(is_low_variety_key(&"abc".repeat(30)));
        assert!(!is_low_variety_key(
            "3f9a1c0e7b52d8e4a6f0c13b9d7e25a8c4f1b06e9d3a7c52e8b0f4a1d6c39e72"
        ));
    }

    #[test]
    fn init_cookie_signing_key_uses_configured_bytes() {
        let raw = "3f9a1c0e7b52d8e4a6f0c13b9d7e25a8c4f1b06e9d3a7c52e8b0f4a1d6c39e72";
        let mut settings = test_settings();
        settings.registry.cookie_signing_key = Some(raw.to_string());

        let key = init_cookie_signing_key(&settings);

        // Existing deployments rely on the raw key bytes, changing the
        // derivation would log everyone out.
        assert_eq!(key.master(), raw.as_bytes());
    }

    #[tokio::test]
    async fn init_docs_storage_falls_back_to_local_disk_when_bucket_unset() {
        let tmp = tempfile::tempdir().unwrap();
        let mut settings = test_settings();
        settings.registry.data_dir = tmp.path().to_str().unwrap().to_string();
        settings.s3.enabled = true;
        settings.s3.docs_bucket = None;

        let docs_storage = init_docs_storage(&settings);
        let key = DocsStorage::file_key("my-crate", "1.0.0", "doc/index.html");
        docs_storage
            .put(&key, Bytes::from_static(b"hi"))
            .await
            .unwrap();

        let expected_path = Path::new(&settings.docs_path()).join("my-crate/1.0.0/doc/index.html");
        assert!(expected_path.exists());
    }
}
