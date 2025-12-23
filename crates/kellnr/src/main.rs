use appstate::AppStateData;
use axum_extra::extract::cookie::Key;
use common::cratesio_prefetch_msg::CratesioPrefetchMsg;
use db::{ConString, Database, DbProvider, PgConString, SqliteConString};
use index::cratesio_prefetch_api::{
    CratesIoPrefetchArgs, UPDATE_CACHE_TIMEOUT_SECS, init_cratesio_prefetch_thread,
};
use moka::future::Cache;
use settings::{LogFormat, Settings};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use storage::{
    cached_crate_storage::DynStorage, cratesio_crate_storage::CratesIoCrateStorage,
    fs_storage::FSStorage, kellnr_crate_storage::KellnrCrateStorage, s3_storage::S3Storage,
};
use tokio::{fs::create_dir_all, net::TcpListener};
use tracing::info;
use tracing_subscriber::fmt::format;

mod routes;

#[tokio::main]
async fn main() {
    let settings: Arc<Settings> = settings::get_settings().expect("Cannot read config").into();
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
    // as the needed folders for the sqlite database my not been created before that.
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
    };

    init_cratesio_prefetch_thread(
        cratesio_prefetch_sender.clone(),
        settings.proxy.num_threads as usize,
        prefetch_args,
    );

    // Docs hosting
    init_docs_hosting(&settings, crate_storage.clone(), db.clone()).await;

    // Webhook support
    init_webhook_service(db.clone());

    let data_dir = settings.registry.data_dir.clone();
    let signing_key = Key::generate();
    let max_docs_size = settings.docs.max_size;
    let max_crate_size = settings.registry.max_crate_size as usize;
    let state = AppStateData {
        db,
        signing_key,
        settings,
        crate_storage,
        cratesio_storage,
        cratesio_prefetch_sender,
    };

    // Create router using the route module
    let app = routes::create_router(state, &data_dir, max_docs_size, max_crate_size);

    // Start the server
    let listener = TcpListener::bind(addr)
        .await
        .unwrap_or_else(|_| panic!("Failed to bind to {addr}"));
    axum::serve(listener, app).await.unwrap();
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
        docs::doc_queue::doc_extraction_queue(db, cs, settings.docs_path());
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
    webhooks::run_webhook_service(db);
}
