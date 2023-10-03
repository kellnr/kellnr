use appstate::AppStateData;
use axum::routing::{delete, post};
use axum::{routing::get, Router};
use axum_extra::extract::cookie::Key;
use common::cratesio_prefetch_msg::CratesioPrefetchMsg;
use common::storage::Storage;
use db::DbProvider;
use db::{ConString, Database, PgConString, SqliteConString};
use index::cratesio_idx::CratesIoIdx;
use index::cratesio_prefetch_api::{background_update_thread, cratesio_prefetch_thread};
use index::kellnr_idx::KellnrIdx;
use index::rwindex::RoIndex;
use index::rwindex::RwIndex;
use registry::cratesio_crate_storage::CratesIoCrateStorage;
use registry::kellnr_crate_storage::KellnrCrateStorage;
use rocket::config::{Config, SecretKey};
use rocket::fs::FileServer;
use rocket::tokio::fs::create_dir_all;
use rocket::tokio::sync::{Mutex, RwLock};
use rocket::{catchers, routes, tokio, Build};
use rocket_cors::{Cors, CorsOptions};
use settings::{LogFormat, Settings};
use std::convert::TryFrom;
use std::path::Path;
use std::sync::Arc;
use std::{process, process::Stdio};
use sysinfo::{System, SystemExt};
use tracing::{debug, info};
use tracing_subscriber::fmt::format;
use web_ui::{ui, user};

// #[launch]
// async fn rocket_launch() -> _ {
//     let settings = Settings::try_from(Path::new("config")).expect("Cannot read config");
//
//     // Configure tracing subscriber
//     init_tracing(&settings);
//
//     info!("Starting kellnr");
//
//     // Initialize kellnr crate storage
//     let kellnr_crate_storage = init_kellnr_crate_storage(&settings).await;
//
//     // Kellnr Index and Storage
//     let kellnr_idx = init_kellnr_git_index(&settings).await;
//
//     // Create the database connection. Has to be done after the index and storage
//     // as the needed folders for the sqlite database my not been created before that.
//     let con_string = get_connect_string(&settings);
//     let db = Database::new(&con_string)
//         .await
//         .expect("Failed to create database");
//     let db = Box::new(db) as Box<dyn DbProvider>;
//
//     // Start git daemon to service the indices
//     // Has to be done, before the crates.io index gets cloned, as the container script kills
//     // the container if the daemon is not running, which happens if the daemon is not started due the
//     // clone process of the crates.io proxy
//     if settings.git_index {
//         start_git_daemon(&settings);
//     }
//
//     // Crates.io Proxy
//     let (cratesio_crate_storage, cratesio_idx) = init_cratesio_proxy(&settings).await;
//     let (cratesio_prefetch_sender, cratesio_prefetch_receiver) =
//         flume::unbounded::<CratesioPrefetchMsg>();
//     let cratesio_prefetch_sender = Arc::new(cratesio_prefetch_sender);
//     let cratesio_prefetch_receiver = Arc::new(cratesio_prefetch_receiver);
//
//     init_cratesio_prefetch_thread(
//         get_connect_string(&settings),
//         cratesio_prefetch_sender.clone(),
//         cratesio_prefetch_receiver,
//         settings.crates_io_num_threads,
//     )
//     .await;
//
//     // Docs hosting
//     init_docs_hosting(&settings, &con_string).await;
//
//     // Start Kellnr
//     build_rocket(
//         settings,
//         db,
//         kellnr_idx,
//         kellnr_crate_storage,
//         cratesio_idx,
//         cratesio_crate_storage,
//         cratesio_prefetch_sender.clone(),
//     )
// }

#[tokio::main]
async fn main() {
    let settings = Settings::try_from(Path::new("config")).expect("Cannot read config");

    // Configure tracing subscriber
    init_tracing(&settings);

    info!("Starting kellnr");

    // Initialize kellnr crate storage
    let kellnr_crate_storage = init_kellnr_crate_storage(&settings).await;

    // Kellnr Index and Storage
    let kellnr_idx = init_kellnr_git_index(&settings).await;

    // Create the database connection. Has to be done after the index and storage
    // as the needed folders for the sqlite database my not been created before that.
    let con_string = get_connect_string(&settings);
    let db = Database::new(&con_string)
        .await
        .expect("Failed to create database");
    let db = Box::new(db) as Box<dyn DbProvider>;

    // Start git daemon to service the indices
    // Has to be done, before the crates.io index gets cloned, as the container script kills
    // the container if the daemon is not running, which happens if the daemon is not started due the
    // clone process of the crates.io proxy
    if settings.git_index {
        start_git_daemon(&settings);
    }

    // Crates.io Proxy
    let (cratesio_crate_storage, cratesio_idx) = init_cratesio_proxy(&settings).await;
    let (cratesio_prefetch_sender, cratesio_prefetch_receiver) =
        flume::unbounded::<CratesioPrefetchMsg>();
    let cratesio_prefetch_sender = Arc::new(cratesio_prefetch_sender);
    let cratesio_prefetch_receiver = Arc::new(cratesio_prefetch_receiver);

    init_cratesio_prefetch_thread(
        get_connect_string(&settings),
        cratesio_prefetch_sender.clone(),
        cratesio_prefetch_receiver,
        settings.crates_io_num_threads,
    )
    .await;

    // Docs hosting
    init_docs_hosting(&settings, &con_string).await;

    let signing_key = Key::generate();
    let state = Arc::new(AppStateData {
        db,
        signing_key,
        settings,
    });

    let user = Router::new()
        .route("/login", post(user::login))
        .route("/logout", get(user::logout))
        .route("/changepwd", post(user::change_pwd))
        .route("/add", post(user::add))
        .route("/delete/:name", delete(user::delete))
        // TODO(ItsEthra): Consider post?
        .route("/resetpwd/:name", get(user::reset_pwd))
        // TODO(ItsEthra): Consider put?
        .route("/addtoken", post(user::add_token))
        // TODO(ItsEthra): Consider delete?
        .route("/delete_token/:id", get(user::delete_token))
        .route("/list_tokens", get(user::list_tokens))
        .route("/list_users", get(user::list_users))
        .route("/login_state", get(user::login_state));

    let app = Router::new()
        .route("/version", get(ui::kellnr_version))
        .route("/crates", get(ui::crates))
        .route("/search", get(ui::search))
        .route("/statistic", get(ui::statistic))
        .nest("/user", user)
        .with_state(state);

    axum::Server::bind(&"0.0.0.0:8000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
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
    let ts = tracing_subscriber::fmt().with_max_level(settings.log_level)
    .with_env_filter(format!("{},mio::poll=error,want=error,sqlx::query=error,sqlx::postgres=warn,sea_orm_migration=warn,cargo=error,globset=warn,hyper=warn,_=warn,reqwest=warn,rocket::server={},rocket::launch={},rocket::shield::shield={},rocket:launch_={}",
                             settings.log_level, settings.log_level_rocket, settings.log_level_rocket, settings.log_level_rocket, settings.log_level_rocket));

    match settings.log_format {
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
    if settings.rustdoc_auto_gen {
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

async fn init_cratesio_proxy(settings: &Settings) -> (CratesIoCrateStorage, Box<dyn RoIndex>) {
    let cratesio_idx_storage = Storage::new();
    let mut cratesio_idx = CratesIoIdx::new(settings, cratesio_idx_storage);
    let cratesio_crate_storage = CratesIoCrateStorage::new(settings)
        .await
        .expect("Failed to create crates.io crate storage.");

    if settings.crates_io_proxy && settings.git_index {
        cratesio_idx
            .start()
            .await
            .expect("Failed to start crates.io index.");
    }

    let cratesio_idx = Box::new(cratesio_idx) as Box<dyn RoIndex>;
    (cratesio_crate_storage, cratesio_idx)
}

async fn init_kellnr_crate_storage(settings: &Settings) -> KellnrCrateStorage {
    KellnrCrateStorage::new(settings)
        .await
        .expect("Failed to create crate storage.")
}

async fn init_kellnr_git_index(settings: &Settings) -> Box<dyn RwIndex> {
    let kellnr_idx_storage = Storage::new();
    let kellnr_idx = KellnrIdx::new(settings, kellnr_idx_storage)
        .await
        .expect("Failed to create index.");
    Box::new(kellnr_idx) as Box<dyn RwIndex>
}

#[derive(Debug)]
enum Environment {
    Debug,
    Release,
}

#[allow(clippy::too_many_arguments)]
pub fn build_rocket(
    settings: Settings,
    db: Box<dyn DbProvider>,
    kellnr_idx: Box<dyn RwIndex>,
    kellnr_crate_storage: KellnrCrateStorage,
    cratesio_idx: Box<dyn RoIndex>,
    cratesio_crate_storage: CratesIoCrateStorage,
    cratesio_prefetch_sender: Arc<flume::Sender<CratesioPrefetchMsg>>,
) -> rocket::Rocket<Build> {
    let env = match Config::default().profile.to_string().as_str() {
        "debug" => Environment::Debug,
        _ => Environment::Release,
    };

    let rocket_conf = Config {
        port: settings.api_port,
        address: settings.web_address,
        secret_key: SecretKey::generate().expect("Unable to create a secret key."),
        ..Config::default()
    };

    rocket::custom(rocket_conf)
        .mount(
            "/",
            routes![
                registry::kellnr_api::me,
                web_ui::settings::settings,
                ui::crate_data,
                ui::cratesio_data,
                ui::delete,
            ],
        )
        .mount("/", FileServer::from("./static"))
        .mount("/docs", FileServer::from(settings.docs_path()).rank(-1))
        .mount(
            "/api/v1/docs",
            routes![
                docs::api::publish_docs,
                docs::api::docs_in_queue,
                ui::build_rustdoc
            ],
        )
        .mount(
            "/user",
            routes![
                // user::login,
                // user::logout,
                // user::change_pwd,
                // user::add,
                // user::delete,
                // user::delete_forbidden, not needed, we use assert in delete now
                // user::reset_pwd,
                // user::add_token,
                // user::delete_token,
                // user::list_tokens,
                // user::list_users,
                // user::login_state,
            ],
        )
        .mount(
            "/api/v1/crates",
            routes![
                index::kellnr_prefetch_api::prefetch_kellnr,
                index::kellnr_prefetch_api::prefetch_len2_kellnr,
                index::kellnr_prefetch_api::config_kellnr,
                registry::kellnr_api::download,
                registry::kellnr_api::publish,
                registry::kellnr_api::yank,
                registry::kellnr_api::unyank,
                registry::kellnr_api::search,
                registry::kellnr_api::list_owners,
                registry::kellnr_api::add_owner,
                registry::kellnr_api::remove_owner,
            ],
        )
        .mount(
            "/api/v1/cratesio",
            routes![
                index::cratesio_prefetch_api::prefetch_cratesio,
                index::cratesio_prefetch_api::prefetch_len2_cratesio,
                index::cratesio_prefetch_api::config_cratesio,
                registry::cratesio_api::download,
                registry::cratesio_api::search,
                ui::delete_cratesio_index
            ],
        )
        .register("/", catchers![ui::not_found])
        .manage(settings)
        .manage(db)
        .manage(RwLock::new(kellnr_crate_storage))
        .manage(Mutex::new(kellnr_idx))
        .manage(Mutex::new(cratesio_idx))
        .manage(RwLock::new(cratesio_crate_storage))
        .manage(cratesio_prefetch_sender)
        .attach(get_cors_header(env))
}

fn get_cors_header(env: Environment) -> Cors {
    use rocket::http::Method;
    match env {
        Environment::Debug => CorsOptions {
            allowed_methods: vec![Method::Get, Method::Post, Method::Options]
                .into_iter()
                .map(From::from)
                .collect(),
            allow_credentials: true,
            ..Default::default()
        },
        Environment::Release => CorsOptions {
            allowed_methods: Vec::<Method>::new().into_iter().map(From::from).collect(),
            ..Default::default()
        },
    }
    .to_cors()
    .expect("Unable to create CORS header")
}

fn start_git_daemon(settings: &Settings) {
    // Check if git-daemon is running. If so, do not start another instance.
    let s = System::new_all();
    if s.processes_by_exact_name("git-daemon").count() > 0 {
        debug!("git-daemon already running. No need to start a new instance.");
        return;
    }

    // start the git daemon
    process::Command::new("git")
        .args([
            "daemon",
            &format!("--base-path={}", settings.base_path().to_string_lossy()),
            // &settings.base_path().to_string_lossy(),
            &format!("--listen={}", settings.index_address),
            &format!("--port={}", settings.index_port),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .spawn()
        .expect("Unable to start index. Please make sure there is not git-daemon running already.");
}
