use appstate::AppStateData;
use axum::{
    extract::DefaultBodyLimit,
    middleware,
    routing::{delete, get, get_service, post, put},
    Router,
};
use axum_extra::extract::cookie::Key;
use common::cratesio_prefetch_msg::CratesioPrefetchMsg;
use db::{ConString, Database, DbProvider, PgConString, SqliteConString};
use index::{
    cratesio_prefetch_api::{self, background_update_thread, cratesio_prefetch_thread},
    kellnr_prefetch_api,
};
use registry::{cratesio_api, kellnr_api};
use settings::{LogFormat, Settings};
use std::{
    convert::TryFrom,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
};
use storage::{
    cratesio_crate_storage::CratesIoCrateStorage, kellnr_crate_storage::KellnrCrateStorage,
};
use tokio::{fs::create_dir_all, net::TcpListener};
use tower_http::services::{ServeDir, ServeFile};
use tracing::info;
use tracing_subscriber::fmt::format;
use web_ui::{session, ui, user};

#[tokio::main]
async fn main() {
    let settings: Arc<Settings> = Settings::try_from(Path::new("config"))
        .expect("Cannot read config")
        .into();
    let addr: SocketAddr = format!("{}:{}", settings.local.ip, settings.local.port)
        .parse()
        .expect("Failed to parse IP and port: {settings.local.ip}:{settings.local.port}");

    // Configure tracing subscriber
    init_tracing(&settings);

    info!("Starting kellnr");

    // Initialize kellnr crate storage
    let crate_storage: Arc<KellnrCrateStorage> = init_kellnr_crate_storage(&settings).await.into();

    // Create the database connection. Has to be done after the index and storage
    // as the needed folders for the sqlite database my not been created before that.
    let con_string = get_connect_string(&settings);
    let db = Database::new(&con_string)
        .await
        .expect("Failed to create database");
    let db = Arc::new(db) as Arc<dyn DbProvider>;

    // Crates.io Proxy
    let cratesio_storage: Arc<CratesIoCrateStorage> = init_cratesio_proxy(&settings).await.into();
    let (cratesio_prefetch_sender, cratesio_prefetch_receiver) =
        flume::unbounded::<CratesioPrefetchMsg>();
    let cratesio_prefetch_sender = Arc::new(cratesio_prefetch_sender);
    let cratesio_prefetch_receiver = Arc::new(cratesio_prefetch_receiver);

    init_cratesio_prefetch_thread(
        get_connect_string(&settings),
        cratesio_prefetch_sender.clone(),
        cratesio_prefetch_receiver,
        settings.proxy.num_threads,
    )
    .await;

    // Docs hosting
    init_docs_hosting(&settings, &con_string).await;
    let data_dir = settings.registry.data_dir.clone();
    let signing_key = Key::generate();
    let max_docs_size = settings.docs.max_size;
    let state = AppStateData {
        db,
        signing_key,
        settings,
        crate_storage,
        cratesio_storage,
        cratesio_prefetch_sender,
    };

    let user = Router::new()
        .route("/login", post(user::login))
        .route("/logout", get(user::logout))
        .route("/change_pwd", post(user::change_pwd))
        .route("/add", post(user::add))
        .route("/delete/:name", delete(user::delete))
        .route("/reset_pwd/:name", post(user::reset_pwd))
        .route("/add_token", post(user::add_token))
        .route("/delete_token/:id", delete(user::delete_token))
        .route("/list_tokens", get(user::list_tokens))
        .route("/list_users", get(user::list_users))
        .route("/login_state", get(user::login_state));

    let docs = Router::new()
        .route("/build", post(ui::build_rustdoc))
        .route("/queue", get(docs::api::docs_in_queue))
        .route(
            "/:package/:version",
            put(docs::api::publish_docs).layer(DefaultBodyLimit::max(max_docs_size * 1_000_000))
        );

    let docs_service = get_service(ServeDir::new(format!("{}/docs", data_dir))).route_layer(
        middleware::from_fn_with_state(state.clone(), session::session_auth_when_required),
    );
    let static_files_service = get_service(
        ServeDir::new(PathBuf::from("static"))
            .append_index_html_on_directories(true)
            .fallback(ServeFile::new(PathBuf::from("static/index.html"))),
    );

    let kellnr_api = Router::new()
        .route("/:crate_name/owners", delete(kellnr_api::remove_owner))
        .route("/:crate_name/owners", put(kellnr_api::add_owner))
        .route("/:crate_name/owners", get(kellnr_api::list_owners))
        .route("/", get(kellnr_api::search))
        .route("/:package/:version/download", get(kellnr_api::download))
        .route("/new", put(kellnr_api::publish))
        .route("/:crate_name/:version/yank", delete(kellnr_api::yank))
        .route("/:crate_name/:version/unyank", put(kellnr_api::unyank))
        .route("/config.json", get(kellnr_prefetch_api::config_kellnr))
        .route("/:a/:b/:package", get(kellnr_prefetch_api::prefetch_kellnr))
        .route(
            "/:a/:package",
            get(kellnr_prefetch_api::prefetch_len2_kellnr),
        )
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::auth_req_token::cargo_auth_when_required,
        ));

    let cratesio_api = Router::new()
        .route("/", get(cratesio_api::search))
        .route("/:package/:version/download", get(cratesio_api::download))
        .route("/config.json", get(cratesio_prefetch_api::config_cratesio))
        .route(
            "/:a/:b/:name",
            get(cratesio_prefetch_api::prefetch_cratesio),
        )
        .route(
            "/:a/:name",
            get(cratesio_prefetch_api::prefetch_len2_cratesio),
        )
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::auth_req_token::cargo_auth_when_required,
        ));

    let ui = Router::new()
        .route("/version", get(ui::kellnr_version))
        .route("/crates", get(ui::crates))
        .route("/search", get(ui::search))
        .route("/statistic", get(ui::statistic))
        .route("/crate_data", get(ui::crate_data))
        .route("/cratesio_data", get(ui::cratesio_data))
        .route("/delete_crate", delete(ui::delete))
        .route("/settings", get(ui::settings))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            session::session_auth_when_required,
        ));

    let app = Router::new()
        .route("/me", get(kellnr_api::me))
        .nest("/api/v1/ui", ui)
        .nest("/api/v1/user", user)
        .nest("/api/v1/docs", docs)
        .nest("/api/v1/crates", kellnr_api)
        .nest("/api/v1/cratesio", cratesio_api)
        .nest_service("/docs", docs_service)
        .fallback(static_files_service)
        .with_state(state)
        .layer(tower_http::trace::TraceLayer::new_for_http());

    let listener = TcpListener::bind(addr)
        .await
        .unwrap_or_else(|_| panic!("Failed to bind to {addr}"));
    axum::serve(listener, app).await.unwrap();
}

async fn init_cratesio_prefetch_thread(
    con_string: ConString,
    sender: Arc<flume::Sender<CratesioPrefetchMsg>>,
    recv: Arc<flume::Receiver<CratesioPrefetchMsg>>,
    num_threads: usize,
) {
    // Threads that takes messages to update the crates.io index
    let db = Arc::new(
        Database::new(&con_string)
            .await
            .expect("Failed to create database connection for crates.io prefetch thread"),
    );
    for _ in 0..num_threads {
        let recv2 = recv.clone();
        let db2 = db.clone();
        tokio::spawn(async move {
            cratesio_prefetch_thread(db2, recv2).await;
        });
    }

    // Thread that periodically checks if the crates.io index needs to be updated.
    // It sends an update message to the thread above which then updates the index.
    tokio::spawn(async move {
        let db = Database::new(&con_string)
            .await
            .expect("Failed to create database connection for crates.io update thread");
        background_update_thread(db, sender).await;
    });
}

fn init_tracing(settings: &Settings) {
    let ts = tracing_subscriber::fmt().with_max_level(settings.log.level)
    .with_env_filter(format!("{},mio::poll=error,want=error,sqlx::query=error,sqlx::postgres=warn,sea_orm_migration=warn,cargo=error,globset=warn,hyper=warn,_=warn,reqwest=warn,tower_http={}", settings.log.level, settings.log.level_web_server));

    match settings.log.format {
        LogFormat::Compact => ts.event_format(format().compact()).init(),
        LogFormat::Pretty => ts.event_format(format().pretty()).init(),
        LogFormat::Json => ts.event_format(format().json()).init(),
    };
}

fn get_connect_string(settings: &Settings) -> ConString {
    if settings.postgresql.enabled {
        ConString::Postgres(PgConString::from(settings))
    } else {
        ConString::Sqlite(SqliteConString::from(settings))
    }
}

async fn init_docs_hosting(settings: &Settings, con_string: &ConString) {
    create_dir_all(settings.docs_path())
        .await
        .expect("Failed to create docs directory.");
    if settings.docs.enabled {
        docs::doc_queue::doc_extraction_queue(
            Database::new(con_string)
                .await
                .expect("Failed to create database"),
            KellnrCrateStorage::new(settings)
                .await
                .expect("Failed to create crate storage."),
            settings.docs_path(),
        )
        .await;
    }
}

async fn init_cratesio_proxy(settings: &Settings) -> CratesIoCrateStorage {
    CratesIoCrateStorage::new(settings)
        .await
        .expect("Failed to create crates.io crate storage.")
}

async fn init_kellnr_crate_storage(settings: &Settings) -> KellnrCrateStorage {
    KellnrCrateStorage::new(settings)
        .await
        .expect("Failed to create crate storage.")
}
