use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use axum_extra::extract::cookie::Key;
use kellnr_appstate::AppStateData;
use kellnr_auth::oauth2::OAuth2Handler;
use kellnr_common::cratesio_prefetch_msg::CratesioPrefetchMsg;
use kellnr_common::token_cache::TokenCacheManager;
use kellnr_db::{ConString, Database, DbProvider, PgConString, SqliteConString};
use kellnr_index::cratesio_prefetch_api::{
    CratesIoPrefetchArgs, UPDATE_CACHE_TIMEOUT_SECS, init_cratesio_prefetch_thread,
};
use kellnr_settings::{CliResult, LogFormat, Settings, parse_cli};
use kellnr_storage::cached_crate_storage::DynStorage;
use kellnr_storage::cratesio_crate_storage::CratesIoCrateStorage;
use kellnr_storage::fs_storage::FSStorage;
use kellnr_storage::kellnr_crate_storage::KellnrCrateStorage;
use kellnr_storage::s3_storage::S3Storage;
use kellnr_storage::toolchain_storage::ToolchainStorage;
use moka::future::Cache;
use tokio::fs::create_dir_all;
use tokio::net::TcpListener;
use tracing::{error, info, warn};
use tracing_subscriber::fmt::format;

mod openapi;
mod routes;

#[tokio::main]
async fn main() {
    let cli_result = parse_cli().expect("Cannot read config");

    match cli_result {
        CliResult::ShowConfig(settings) => {
            show_config(&settings);
        }
        CliResult::InitConfig { settings, output } => {
            init_config(&settings, &output);
        }
        CliResult::RunServer(settings) => {
            run_server(settings).await;
        }
        CliResult::ShowHelp => {
            // Help was already printed by parse_cli()
        }
    }
}

fn show_config(settings: &Settings) {
    match toml::to_string_pretty(settings) {
        Ok(toml) => println!("{toml}"),
        Err(e) => {
            eprintln!("Error serializing config: {e}");
            std::process::exit(1);
        }
    }
}

fn init_config(settings: &Settings, output: &Path) {
    if output.exists() {
        eprintln!("Error: File already exists: {}", output.display());
        eprintln!("Remove the file or specify a different output path with -o");
        std::process::exit(1);
    }

    match toml::to_string_pretty(settings) {
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

async fn run_server(settings: Settings) {
    let settings: Arc<Settings> = settings.into();

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

    let prefetch_args = CratesIoPrefetchArgs {
        db: db.clone(),
        cache: Cache::builder()
            .time_to_live(Duration::from_secs(UPDATE_CACHE_TIMEOUT_SECS))
            .build(),
        recv: cratesio_prefetch_receiver.clone(),
        download_on_update: settings.proxy.download_on_update,
        storage: cratesio_storage.clone(),
        url: settings.proxy.url.clone(),
        index: settings.proxy.index.clone(),
    };

    init_cratesio_prefetch_thread(
        cratesio_prefetch_sender.clone(),
        settings.proxy.num_threads,
        prefetch_args,
    );

    // Docs hosting
    init_docs_hosting(&settings, crate_storage.clone(), db.clone()).await;

    // Webhook support
    init_webhook_service(db.clone());

    let data_dir = settings.registry.data_dir.clone();
    let signing_key = init_cookie_signing_key(&settings);
    let max_docs_size = settings.docs.max_size;
    let max_crate_size = settings.registry.max_crate_size as usize;
    let max_toolchain_size = settings.toolchain.max_size;
    let route_path_prefix = settings.origin.path.trim().to_owned();
    let token_cache = Arc::new(TokenCacheManager::new(
        settings.registry.token_cache_enabled,
        settings.registry.token_cache_ttl_seconds,
        settings.registry.token_cache_max_capacity,
    ));

    // Initialize toolchain storage if enabled
    let toolchain_storage = init_toolchain_storage(&settings);

    // Initialize OAuth2/OIDC handler if enabled
    let oauth2_handler = init_oauth2_handler(&settings).await;

    let state = AppStateData {
        db,
        signing_key,
        settings,
        crate_storage,
        cratesio_storage,
        cratesio_prefetch_sender,
        token_cache,
        toolchain_storage,
    };

    // Create router using the route module
    let mut app = routes::create_router(
        state,
        &data_dir,
        max_docs_size,
        max_crate_size,
        max_toolchain_size,
        oauth2_handler,
    );
    if !route_path_prefix.is_empty() {
        app = axum::Router::new().nest(&route_path_prefix, app);
    }

    // Start the server
    let listener = TcpListener::bind(addr)
        .await
        .unwrap_or_else(|_| panic!("Failed to bind to {addr}"));
    info!("Kellnr has been started on http://{addr}/.");
    axum::serve(listener, app).await.unwrap();
}

fn init_cookie_signing_key(settings: &Settings) -> Key {
    // Either take the provided signing key or generate a random one.
    // A provided key can be shared between multiple instances of kellnr to
    // allow UI authentication in multi-instance scenarios.
    if let Some(key) = &settings.registry.cookie_signing_key {
        Key::try_from(key.as_bytes()).expect("Failed to create cookie signing key from provided settings. The key has to be at least 64 bytes long.")
    } else {
        Key::generate()
    }
}

fn init_tracing(settings: &Settings) {
    let ts = tracing_subscriber::fmt()
        .with_max_level(settings.log.level)
        .with_env_filter(format!(
            "{},mio::poll=error,want=error,sqlx::query=error,sqlx::postgres=warn,\
                sea_orm_migration=warn,cargo=error,globset=warn,\
                hyper=warn,_=warn,reqwest=warn,tower_http={},\
                object_store::aws::builder=error",
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

async fn init_docs_hosting(
    settings: &Settings,
    cs: Arc<KellnrCrateStorage>,
    db: Arc<dyn DbProvider + 'static>,
) {
    create_dir_all(settings.docs_path())
        .await
        .expect("Failed to create docs directory.");
    if settings.docs.enabled {
        kellnr_docs::doc_queue::doc_extraction_queue(
            db,
            cs,
            settings.docs_path(),
            settings.origin.path.clone(),
        );
    }
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
    if settings.s3.enabled {
        let s = S3Storage::try_from((folder, settings)).expect("Failed to create S3 storage.");
        Box::new(s) as DynStorage
    } else {
        let s = FSStorage::new(folder).expect("Failed to create FS storage.");
        Box::new(s) as DynStorage
    }
}

fn init_webhook_service(db: Arc<dyn DbProvider + 'static>) {
    kellnr_webhooks::run_webhook_service(db);
}

fn init_toolchain_storage(settings: &Arc<Settings>) -> Option<Arc<ToolchainStorage>> {
    if !settings.toolchain.enabled {
        return None;
    }

    let storage = init_storage(&settings.toolchain_path_or_bucket(), settings);
    let toolchain_storage = ToolchainStorage::new(storage);

    Some(Arc::new(toolchain_storage))
}

async fn init_oauth2_handler(settings: &Settings) -> Option<Arc<OAuth2Handler>> {
    if !settings.oauth2.enabled {
        return None;
    }

    // Construct the callback URL based on settings
    let protocol = &settings.origin.protocol; // Protocol enum implements Display
    let host = &settings.origin.hostname;
    let port = settings.origin.port;
    let path_prefix = settings.origin.path.trim();

    let callback_url = if port == 443 || port == 80 {
        format!("{protocol}://{host}{path_prefix}/api/v1/oauth2/callback")
    } else {
        format!("{protocol}://{host}:{port}{path_prefix}/api/v1/oauth2/callback")
    };

    match OAuth2Handler::from_discovery(&settings.oauth2, &callback_url).await {
        Ok(handler) => Some(Arc::new(handler)),
        Err(e) => {
            error!("Failed to initialize OAuth2/OIDC handler: {}", e);
            warn!("OAuth2/OIDC authentication will be disabled");
            None
        }
    }
}
