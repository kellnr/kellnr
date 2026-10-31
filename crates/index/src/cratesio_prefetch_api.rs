use std::sync::Arc;
use std::time::Duration;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use hyper::StatusCode;
use kellnr_common::base_url::BaseUrl;
use kellnr_common::cratesio_prefetch_msg::{CratesioPrefetchMsg, InsertData, UpdateData};
use kellnr_common::index_metadata::IndexMetadata;
use kellnr_common::original_name::OriginalName;
use kellnr_common::prefetch::Prefetch;
use kellnr_common::version::Version;
use kellnr_cratesio_sync::cratesio_downloader::{download_crate, has_checksum};
use kellnr_cratesio_sync::{CratesioSync, SyncSource, crate_sub_path};
use kellnr_db::DbProvider;
use kellnr_db::provider::PrefetchState;
use kellnr_filter::{CrateInfo, FilterChain, Verdict};
use kellnr_http::error::ApiResult;
use kellnr_http::state::{CratesIoPrefetchSenderState, DbState, SettingsState};
use kellnr_storage::cratesio_crate_storage::CratesIoCrateStorage;
use moka::future::Cache;
use reqwest::Client;
use tracing::{error, trace, warn};

use super::config_json::{ConfigJson, IndexKind};
use crate::prefetch_response::PrefetchResponse;

pub static UPDATE_INTERVAL_SECS: u64 = 60 * 120; // 2h background update interval
pub static UPDATE_CACHE_TIMEOUT_SECS: u64 = 60 * 30; // 30 min cache timeout
static UPDATE_LIST_RETRY_SECS: u64 = 60; // retry delay if an update round cannot run
static CLIENT: std::sync::OnceLock<Client> = std::sync::OnceLock::new();

fn get_client(settings: &kellnr_settings::Proxy) -> &'static Client {
    CLIENT.get_or_init(|| {
        kellnr_cratesio_sync::cratesio_downloader::build_client(
            &settings.user_agent,
            Duration::from_secs(settings.connect_timeout_seconds),
            Duration::from_secs(settings.request_timeout_seconds),
        )
    })
}

/// Get crates.io proxy configuration
///
/// Returns the sparse registry configuration for the crates.io proxy.
#[utoipa::path(
    get,
    path = "/config.json",
    tag = "cratesio",
    responses(
        (status = 200, description = "Crates.io proxy configuration")
    ),
    security(("cargo_token" = []))
)]
#[allow(clippy::unused_async)] // part of the router
pub async fn config_cratesio(State(settings): SettingsState) -> Json<ConfigJson> {
    Json(ConfigJson::new(&settings, IndexKind::CratesIo))
}

/// Prefetch crate metadata from crates.io (3+ char names)
///
/// Fetches and caches crate index metadata from crates.io for crates with
/// names of 3 or more characters.
#[utoipa::path(
    get,
    path = "/{a}/{b}/{name}",
    tag = "cratesio",
    params(
        ("a" = String, Path, description = "First two characters of crate name"),
        ("b" = String, Path, description = "Next two characters of crate name"),
        ("name" = String, Path, description = "Full crate name")
    ),
    responses(
        (status = 200, description = "Crate index metadata"),
        (status = 304, description = "Not modified"),
        (status = 404, description = "Crate not found"),
        (status = 502, description = "Crates.io is not reachable")
    ),
    security(("cargo_token" = []))
)]
pub async fn prefetch_cratesio(
    Path((_a, _b, name)): Path<(String, String, OriginalName)>,
    headers: HeaderMap,
    State(db): DbState,
    State(sender): CratesIoPrefetchSenderState,
    State(settings): SettingsState,
) -> ApiResult<PrefetchResponse> {
    PrefetchResponse::from_result(
        internal_prefetch_cratesio(name, headers, &db, &sender, &settings.proxy).await,
    )
}

/// Prefetch crate metadata from crates.io (1-2 char names)
///
/// Fetches and caches crate index metadata from crates.io for crates with
/// names of 1-2 characters.
#[utoipa::path(
    get,
    path = "/{a}/{name}",
    tag = "cratesio",
    params(
        ("a" = String, Path, description = "Length prefix (1 or 2)"),
        ("name" = String, Path, description = "Full crate name")
    ),
    responses(
        (status = 200, description = "Crate index metadata"),
        (status = 304, description = "Not modified"),
        (status = 404, description = "Crate not found"),
        (status = 502, description = "Crates.io is not reachable")
    ),
    security(("cargo_token" = []))
)]
pub async fn prefetch_len2_cratesio(
    Path((_a, name)): Path<(String, OriginalName)>,
    headers: HeaderMap,
    State(db): DbState,
    State(sender): CratesIoPrefetchSenderState,
    State(settings): SettingsState,
) -> ApiResult<PrefetchResponse> {
    PrefetchResponse::from_result(
        internal_prefetch_cratesio(name, headers, &db, &sender, &settings.proxy).await,
    )
}

pub fn init_cratesio_prefetch_thread(
    sender: flume::Sender<CratesioPrefetchMsg>,
    num_threads: usize,
    args: CratesIoPrefetchArgs,
) {
    // Tasks that take messages to update the crates.io index. A task that
    // panics is logged and started again.
    for worker in 0..num_threads {
        let args = args.clone();

        tokio::spawn(async move {
            loop {
                let args = args.clone();
                let task = tokio::spawn(async move {
                    cratesio_prefetch_thread(args).await;
                });
                // The worker never returns, it can only panic.
                let Err(e) = task.await;
                error!("crates.io prefetch worker {worker} stopped, restarting it: {e}");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });
    }

    // Thread that periodically checks if the crates.io index needs to be updated.
    // It sends an update message to the threads above which then updates the index.
    tokio::spawn(async move {
        background_update_thread(args.db.clone(), sender).await;
    });
}

async fn internal_prefetch_cratesio(
    name: OriginalName,
    headers: HeaderMap,
    db: &Arc<dyn DbProvider>,
    sender: &flume::Sender<CratesioPrefetchMsg>,
    proxy_settings: &kellnr_settings::Proxy,
) -> Result<Prefetch, StatusCode> {
    let if_modified_since = headers
        .get("if-modified-since")
        .map(|h| h.to_str().unwrap_or_default().to_string());
    let if_none_match = headers
        .get("if-none-match")
        .map(|h| h.to_str().unwrap_or_default().to_string());

    trace!(
        "Prefetching {name} from crates.io cache: Etag {if_none_match:?} - LM {if_modified_since:?}",
    );

    let prefetch_state = db
        .is_cratesio_cache_up_to_date(&name, if_none_match.clone(), if_modified_since.clone())
        .await
        .map_err(|e| {
            error!("Could not check if cache is up to date for {name}. Error {e}",);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // The client's conditional headers are only used to answer locally. The
    // upstream refresh must use the validators stored in the database, else a
    // client could send the current upstream ETag, get a 304 from crates.io and
    // keep stale index data (e.g. missing yanks) cached.
    match prefetch_state {
        PrefetchState::NeedsUpdate(p) => {
            background_update(
                name.clone(),
                sender,
                p.etag.clone(),
                p.last_modified.clone(),
            );
            trace!("Prefetching {name} from crates.io cache: Needs Update");
            Ok(p)
        }
        PrefetchState::UpToDate {
            etag,
            last_modified,
        } => {
            background_update(name.clone(), sender, etag, last_modified);
            trace!("Prefetching {name} from crates.io cache: Up to Date");
            Err(StatusCode::NOT_MODIFIED)
        }
        PrefetchState::NotFound => {
            let index_url = proxy_settings.index_url().map_err(|e| {
                error!("Invalid proxy.index: {e}");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
            Ok(fetch_cratesio_prefetch(name, &index_url, sender, proxy_settings).await?)
        }
    }
}

fn background_update(
    name: OriginalName,
    sender: &flume::Sender<CratesioPrefetchMsg>,
    stored_etag: String,
    stored_last_modified: String,
) {
    if let Err(e) = sender.send(CratesioPrefetchMsg::Update(UpdateData {
        name,
        etag: Some(stored_etag),
        last_modified: Some(stored_last_modified),
    })) {
        error!("Could not send update message: {e}");
    }
}

async fn background_update_thread(
    db: Arc<dyn DbProvider>,
    sender: flume::Sender<CratesioPrefetchMsg>,
) {
    loop {
        let next_run_secs = queue_update_round(db.as_ref(), &sender).await;
        tokio::time::sleep(Duration::from_secs(next_run_secs)).await;
    }
}

/// Queues an update of all cached crates and returns the seconds until the
/// next round. While messages are still queued, the round is postponed, so
/// rounds do not pile up if the workers cannot keep up.
async fn queue_update_round(
    db: &dyn DbProvider,
    sender: &flume::Sender<CratesioPrefetchMsg>,
) -> u64 {
    if !sender.is_empty() {
        warn!(
            "Postponing the crates.io index update, {} updates are still queued",
            sender.len()
        );
        return UPDATE_LIST_RETRY_SECS;
    }

    match db.get_cratesio_index_update_list().await {
        Ok(crates) => {
            for c in crates {
                if let Err(e) = sender.send(c) {
                    error!("Could not send update message: {e}");
                }
            }
            UPDATE_INTERVAL_SECS
        }
        Err(e) => {
            error!("Could not get crates.io index update list: {e}");
            UPDATE_LIST_RETRY_SECS
        }
    }
}

#[derive(Clone)]
pub struct CratesIoPrefetchArgs {
    pub db: Arc<dyn DbProvider>,
    pub cache: Cache<OriginalName, String>,
    pub recv: flume::Receiver<CratesioPrefetchMsg>,
    pub download_on_update: bool,
    pub url: BaseUrl,
    pub storage: Arc<CratesIoCrateStorage>,
    pub proxy_settings: Arc<kellnr_settings::Proxy>,
    pub sync: CratesioSync,
    pub filters: FilterChain,
}

async fn cratesio_prefetch_thread(args: CratesIoPrefetchArgs) -> ! {
    loop {
        let synced = handle_cratesio_prefetch_msg(&args).await;

        // If the download_on_update flag is set, do not only update the prefetch data in
        // the database, but also download the crates.
        if let Some(metadata) = synced
            && args.download_on_update
        {
            predownload_crates(&metadata, &args).await;
        }
    }
}

/// Download the versions into the cache, except for those the filters deny.
async fn predownload_crates(metadata: &[IndexMetadata], args: &CratesIoPrefetchArgs) {
    for idx in metadata {
        let Ok(version) = Version::try_from(idx.vers.as_str()) else {
            warn!(
                "Skipping download of crate {} with the invalid version {}",
                idx.name, idx.vers
            );
            continue;
        };
        if is_denied(idx, &version, args).await {
            trace!(
                "Crate {} version {} is denied by the filters, skipping download",
                idx.name, idx.vers
            );
        } else {
            predownload_crate(idx, &version, args).await;
        }
    }
}

/// Whether the crates.io filters deny the download of the version.
async fn is_denied(idx: &IndexMetadata, version: &Version, args: &CratesIoPrefetchArgs) -> bool {
    if args.filters.is_empty() {
        return false;
    }
    let name = OriginalName::from_unchecked(idx.name.clone());
    let license = match args
        .db
        .get_cratesio_version_meta(&name.to_canonical(), version)
        .await
    {
        Ok(meta) => meta.and_then(|m| m.license),
        Err(e) => {
            error!(
                "Could not get license of crate {} version {}: {e}",
                idx.name, idx.vers
            );
            None
        }
    };
    let verdict = args.filters.check(&CrateInfo {
        name: &name,
        version,
        license: license.as_deref(),
    });
    verdict != Verdict::Allow
}

async fn predownload_crate(idx: &IndexMetadata, version: &Version, args: &CratesIoPrefetchArgs) {
    let name = OriginalName::from_unchecked(idx.name.clone());

    match args.storage.exists(&name, version).await {
        Ok(true) => {
            trace!(
                "Crate {} version {} already exists in storage, skipping download",
                idx.name, idx.vers
            );
        }
        Ok(false) => {
            trace!("Downloading version {} for crate {}", idx.vers, idx.name);
            match download_crate(
                get_client(&args.proxy_settings),
                &idx.name,
                &idx.vers,
                &args.url,
            )
            .await
            {
                Ok(crate_data) if !has_checksum(&crate_data, &idx.cksum) => {
                    error!(
                        "Checksum mismatch for crate {} version {} from crates.io, not caching it",
                        idx.name, idx.vers
                    );
                }
                Ok(crate_data) => {
                    if let Err(e) = args.storage.put(&name, version, crate_data).await {
                        warn!(
                            "Could not save crate {} version {}: {e}",
                            idx.name, idx.vers
                        );
                    }
                }
                Err(e) => {
                    error!(
                        "Could not download crate {} version {}: {e}",
                        idx.name, idx.vers
                    );
                }
            }
        }
        Err(e) => {
            error!(
                "Could not check if crate {} version {} exists: {e}",
                idx.name, idx.vers
            );
        }
    }
}

/// Handle the next message. Returns the index metadata if the crate was
/// synced.
async fn handle_cratesio_prefetch_msg(args: &CratesIoPrefetchArgs) -> Option<Arc<[IndexMetadata]>> {
    match args.recv.recv_async().await {
        Ok(CratesioPrefetchMsg::Insert(msg)) => {
            trace!("Inserting prefetch data from crates.io for {}", msg.name);
            let date = chrono::Utc::now().to_rfc3339();
            args.cache.insert(msg.name.clone(), date).await;
            args.sync
                .sync(
                    &msg.name,
                    SyncSource::Index {
                        etag: msg.etag,
                        last_modified: msg.last_modified,
                        data: msg.data,
                    },
                )
                .await
                .written()
        }
        Ok(CratesioPrefetchMsg::Update(msg)) => {
            trace!("Updating prefetch data for {}", msg.name);
            if let Some(date) = args.cache.get(&msg.name).await {
                trace!("No update needed for {}. Last update: {date}", msg.name);
                None
            } else {
                args.cache
                    .insert(msg.name.clone(), chrono::Utc::now().to_rfc3339())
                    .await;
                args.sync
                    .sync(
                        &msg.name,
                        SyncSource::Refresh {
                            etag: msg.etag,
                            last_modified: msg.last_modified,
                        },
                    )
                    .await
                    .written()
            }
        }
        Ok(CratesioPrefetchMsg::IncDownloadCnt(msg)) => {
            trace!(
                "Incrementing download count for {} {}",
                msg.name, msg.version
            );
            args.db
                .increase_cached_download_counter(&msg.name, &msg.version)
                .await
                .unwrap_or_else(|e| warn!("Failed to increase download counter: {e}"));
            None
        }
        Err(e) => {
            error!("Could not receive prefetch message: {e}");
            None
        }
    }
}

async fn fetch_cratesio_prefetch(
    name: OriginalName,
    index_url: &BaseUrl,
    sender: &flume::Sender<CratesioPrefetchMsg>,
    proxy_settings: &kellnr_settings::Proxy,
) -> Result<Prefetch, StatusCode> {
    let url = index_url.join(&crate_sub_path(&name));

    let response = get_client(proxy_settings).get(url).send().await;

    match response {
        Ok(r) => {
            match r.status() {
                status @ (StatusCode::NOT_FOUND
                | StatusCode::GONE
                | StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS) => Err(status),

                StatusCode::OK => {
                    let headers = r.headers();
                    let etag = headers
                        .get("ETag")
                        .map(|h| h.to_str().unwrap_or_default().to_string());
                    let last_modified = headers
                        .get("Last-Modified")
                        .map(|h| h.to_str().unwrap_or_default().to_string());

                    let data = r.text().await.map_err(|e| {
                        error!("Could not read prefetch data of {name} from crates.io: {e}");
                        StatusCode::BAD_GATEWAY
                    })?;

                    let prefetch = Prefetch {
                        etag: etag.clone().unwrap_or_default(),
                        last_modified: last_modified.clone().unwrap_or_default(),
                        data: data.clone().into_bytes(),
                    };

                    // Send a message to the prefetch thread to asynchronously update the database.
                    // Else we need to wait for the database to update before we can return the response,
                    // which would take a long time.
                    sender
                        .send(CratesioPrefetchMsg::Insert(InsertData {
                            name,
                            etag,
                            last_modified,
                            data,
                        }))
                        .map_err(|e| {
                            error!("Could not send prefetch message: {e}");
                            StatusCode::INTERNAL_SERVER_ERROR
                        })?;

                    Ok(prefetch)
                }
                // A bad gateway lets cargo retry the request.
                s => {
                    error!("Unexpected status code from crates.io for {name}: {s}");
                    Err(StatusCode::BAD_GATEWAY)
                }
            }
        }
        Err(e) => {
            error!("Error fetching prefetch data from crates.io: {e}");
            Err(StatusCode::BAD_GATEWAY)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::mem;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use axum::Router;
    use axum::body::Body;
    use axum::http::{Request, header};
    use axum::routing::get;
    use http_body_util::BodyExt;
    use kellnr_db::mock::MockDb;
    use kellnr_http::state::AppStateData;
    use kellnr_settings::{Protocol, Settings};
    use reqwest::Url;
    use tower::ServiceExt;

    use super::*;

    #[tokio::test]
    async fn fetch_cratesio_prefetch_works() {
        let r = app()
            .oneshot(
                Request::get("/api/v1/cratesio/ro/ck/rocket")
                    .header(header::IF_MODIFIED_SINCE, "date")
                    .header(header::ETAG, "etag")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = r.status();
        let prefetch = r.into_body().collect().await.unwrap().to_bytes();

        assert_eq!(status, StatusCode::OK);
        assert!(prefetch.len() > 500);
    }

    #[tokio::test]
    async fn fetch_cratesio_prefetch_404() {
        let r = app()
            .oneshot(
                // URL points to crate that does not exist
                Request::get("/api/v1/cratesio/ro/ck/rock123456789")
                    .header(header::IF_MODIFIED_SINCE, "date")
                    .header(header::ETAG, "etag")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = r.status();
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn fetch_cratesio_prefetch_unreachable_upstream_is_bad_gateway() {
        let (sender, _receiver) = flume::unbounded::<CratesioPrefetchMsg>();
        let proxy = kellnr_settings::Proxy::default();
        // Nothing listens on port 1, so the connection is refused.
        let index_url = BaseUrl::new(Url::parse("http://127.0.0.1:1/").unwrap()).unwrap();

        let result = fetch_cratesio_prefetch(
            OriginalName::try_from("rocket").unwrap(),
            &index_url,
            &sender,
            &proxy,
        )
        .await;

        assert_eq!(Err(StatusCode::BAD_GATEWAY), result);
    }

    #[tokio::test]
    async fn config_returns_config_json() {
        let r = app()
            .oneshot(
                Request::get("/api/v1/cratesio/config.json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let result_msg = r.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(
            r#"{"dl":"http://test.api.com:1234/api/v1/cratesio/dl","auth-required":false}"#,
            std::str::from_utf8(&result_msg).unwrap()
        );
    }

    fn conditional_headers() -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::IF_NONE_MATCH, "client-etag".parse().unwrap());
        headers.insert(header::IF_MODIFIED_SINCE, "client-date".parse().unwrap());
        headers
    }

    async fn sent_update(state: PrefetchState) -> (Result<Prefetch, StatusCode>, UpdateData) {
        let mut mock_db = MockDb::new();
        mock_db
            .expect_is_cratesio_cache_up_to_date()
            .withf(|_, etag, lm| {
                etag.as_deref() == Some("client-etag") && lm.as_deref() == Some("client-date")
            })
            .return_once(move |_, _, _| Ok(state));
        let db: Arc<dyn DbProvider> = Arc::new(mock_db);
        let (sender, receiver) = flume::unbounded::<CratesioPrefetchMsg>();
        let proxy = kellnr_settings::Proxy::default();

        let result = internal_prefetch_cratesio(
            OriginalName::try_from("rocket").unwrap(),
            conditional_headers(),
            &db,
            &sender,
            &proxy,
        )
        .await;

        let Ok(CratesioPrefetchMsg::Update(update)) = receiver.try_recv() else {
            panic!("Expected an update message");
        };
        (result, update)
    }

    #[tokio::test]
    async fn background_update_uses_stored_validators_when_stale() {
        let stored = Prefetch {
            data: vec![0x1],
            etag: "stored-etag".to_string(),
            last_modified: "stored-date".to_string(),
        };

        let (result, update) = sent_update(PrefetchState::NeedsUpdate(stored)).await;

        assert!(result.is_ok());
        assert_eq!(Some("stored-etag".to_string()), update.etag);
        assert_eq!(Some("stored-date".to_string()), update.last_modified);
    }

    #[tokio::test]
    async fn background_update_uses_stored_validators_when_up_to_date() {
        let state = PrefetchState::UpToDate {
            etag: "stored-etag".to_string(),
            last_modified: "stored-date".to_string(),
        };

        let (result, update) = sent_update(state).await;

        assert_eq!(Err(StatusCode::NOT_MODIFIED), result.map(|_| ()));
        assert_eq!(Some("stored-etag".to_string()), update.etag);
        assert_eq!(Some("stored-date".to_string()), update.last_modified);
    }

    #[tokio::test(start_paused = true)]
    async fn background_update_thread_backs_off_on_db_error() {
        let calls = Arc::new(AtomicUsize::new(0));
        let mut mock_db = MockDb::new();
        mock_db.expect_get_cratesio_index_update_list().returning({
            let calls = calls.clone();
            move || {
                calls.fetch_add(1, Ordering::SeqCst);
                Err(kellnr_db::error::DbError::FailedToCountCrates)
            }
        });
        let (sender, _receiver) = flume::unbounded::<CratesioPrefetchMsg>();

        let run = background_update_thread(Arc::new(mock_db), sender);
        let window = Duration::from_secs(UPDATE_LIST_RETRY_SECS * 2 + 1);
        assert!(tokio::time::timeout(window, run).await.is_err());

        // Calls at t=0, t=retry and t=2*retry. Without a delay the loop would spin.
        assert_eq!(3, calls.load(Ordering::SeqCst));
    }

    fn app() -> Router {
        let settings = Settings {
            origin: kellnr_settings::Origin {
                protocol: Protocol::Http,
                hostname: "test.api.com".to_string(),
                port: 1234,
                path: String::new(),
                cookie_secure: None,
            },
            ..Settings::default()
        };

        let mut mock_db = MockDb::new();

        mock_db
            .expect_add_cratesio_prefetch_data()
            .returning(move |_, _, _, _, _, _| {
                Ok(Prefetch {
                    data: vec![0x1, 0x2, 0x3],
                    etag: "etag".to_string(),
                    last_modified: "date".to_string(),
                })
            });

        mock_db
            .expect_is_cratesio_cache_up_to_date()
            .returning(move |_, _, _| Ok(PrefetchState::NotFound));

        let (sender, receiver) = flume::unbounded::<CratesioPrefetchMsg>();
        // Make receiver undroppable, else the sender will fail, as the receiver is dropped when
        // this function goes out of scope
        mem::forget(receiver);

        let cratesio_prefetch = Router::new()
            .route("/config.json", get(config_cratesio))
            .route("/{_}/{_}/{name}", get(prefetch_cratesio))
            .route("/{_}/{name}", get(prefetch_len2_cratesio));

        let state = AppStateData {
            db: Arc::new(mock_db),
            settings: Arc::new(settings),
            cratesio_prefetch_sender: sender,
            ..kellnr_http::test_support::test_state()
        };

        Router::new()
            .nest("/api/v1/cratesio", cratesio_prefetch)
            .with_state(state)
    }

    mod worker {
        use std::collections::HashMap;
        use std::sync::Mutex;

        use axum::extract::Path as AxumPath;
        use axum::response::IntoResponse;
        use kellnr_common::license::UNKNOWN_LICENSE;
        use kellnr_db::provider::CratesioVersionMeta;
        use kellnr_settings::CratesioFilter;
        use kellnr_storage::cached_crate_storage::DynStorage;
        use kellnr_storage::fs_storage::FSStorage;

        use super::*;

        const CRATE: &[u8] = b"crate bytes";
        /// Versions of the crate "upstream" with their license.
        const VERSIONS: &[(&str, &str)] = &[("1.0.0", "MIT"), ("1.1.0", "GPL-3.0-only")];

        #[derive(Default)]
        struct Upstream {
            index_calls: AtomicUsize,
            api_calls: AtomicUsize,
            downloads: Mutex<Vec<String>>,
        }

        fn index() -> String {
            let cksum = sha256::digest(CRATE);
            VERSIONS
                .iter()
                .map(|(vers, _)| {
                    format!(
                        r#"{{"name":"upstream","vers":"{vers}","deps":[],"cksum":"{cksum}","features":{{}},"yanked":false}}"#
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        }

        /// A fake crates.io with the index, the API and downloads of "upstream".
        async fn fake_cratesio(upstream: Arc<Upstream>) -> kellnr_settings::Proxy {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            let (index_up, api_up, dl_up) = (upstream.clone(), upstream.clone(), upstream);
            let app = Router::new()
                .route(
                    "/index/{a}/{b}/{name}",
                    get(move || {
                        let upstream = index_up.clone();
                        async move {
                            upstream.index_calls.fetch_add(1, Ordering::SeqCst);
                            ([(header::ETAG, "etag")], index())
                        }
                    }),
                )
                .route(
                    "/api/{name}",
                    get(move || {
                        let upstream = api_up.clone();
                        async move {
                            upstream.api_calls.fetch_add(1, Ordering::SeqCst);
                            let versions: Vec<_> = VERSIONS
                                .iter()
                                .map(|(num, license)| serde_json::json!({"num": num, "license": license}))
                                .collect();
                            serde_json::json!({"crate": {"description": null}, "versions": versions})
                                .to_string()
                        }
                    }),
                )
                .route(
                    "/dl/{name}/{version}/download",
                    get(move |AxumPath((_, version)): AxumPath<(String, String)>| {
                        let upstream = dl_up.clone();
                        async move {
                            upstream.downloads.lock().unwrap().push(version);
                            CRATE.into_response()
                        }
                    }),
                );
            tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

            kellnr_settings::Proxy {
                index: Url::parse(&format!("http://{addr}/index/")).unwrap(),
                api: Url::parse(&format!("http://{addr}/api/")).unwrap(),
                url: Url::parse(&format!("http://{addr}/dl/")).unwrap(),
                ..kellnr_settings::Proxy::default()
            }
        }

        type Stored = Arc<Mutex<Vec<HashMap<String, String>>>>;

        /// A database without stored crates that records the licenses written.
        fn recording_db() -> (MockDb, Stored) {
            let stored: Stored = Arc::default();
            let mut db = MockDb::new();
            db.expect_is_cratesio_cache_up_to_date()
                .returning(|_, _, _| Ok(PrefetchState::NotFound));
            db.expect_add_cratesio_prefetch_data().returning({
                let stored = stored.clone();
                move |_, _, _, _, _, licenses| {
                    stored.lock().unwrap().push(licenses.clone());
                    Ok(Prefetch {
                        data: vec![],
                        etag: String::new(),
                        last_modified: String::new(),
                    })
                }
            });
            (db, stored)
        }

        struct Worker {
            args: CratesIoPrefetchArgs,
            sender: flume::Sender<CratesioPrefetchMsg>,
            _data_dir: tempfile::TempDir,
        }

        fn worker(db: MockDb, proxy: &kellnr_settings::Proxy, filters: FilterChain) -> Worker {
            let data_dir = tempfile::tempdir().unwrap();
            let settings = Settings {
                registry: kellnr_settings::Registry {
                    data_dir: data_dir.path().to_string_lossy().to_string(),
                    ..kellnr_settings::Registry::default()
                },
                ..Settings::default()
            };
            let storage =
                Box::new(FSStorage::new(&settings.crates_io_path()).unwrap()) as DynStorage;
            let db: Arc<dyn DbProvider> = Arc::new(db);
            let (sender, recv) = flume::unbounded();
            let args = CratesIoPrefetchArgs {
                db: db.clone(),
                cache: Cache::builder().build(),
                recv,
                download_on_update: true,
                url: proxy.download_url().unwrap(),
                storage: Arc::new(CratesIoCrateStorage::new(&settings, storage)),
                proxy_settings: Arc::new(proxy.clone()),
                sync: CratesioSync::new(db, Client::new(), proxy),
                filters,
            };
            Worker {
                args,
                sender,
                _data_dir: data_dir,
            }
        }

        fn upstream_name() -> OriginalName {
            OriginalName::try_from("upstream").unwrap()
        }

        fn metadata() -> Vec<IndexMetadata> {
            index()
                .lines()
                .map(|l| serde_json::from_str(l).unwrap())
                .collect()
        }

        async fn is_cached(worker: &Worker, version: &str) -> bool {
            worker
                .args
                .storage
                .exists(&upstream_name(), &Version::try_from(version).unwrap())
                .await
                .unwrap()
        }

        #[tokio::test]
        async fn predownload_skips_versions_denied_by_filters() {
            let upstream = Arc::new(Upstream::default());
            let proxy = fake_cratesio(upstream.clone()).await;
            let mut db = MockDb::new();
            db.expect_get_cratesio_version_meta()
                .returning(|_, version| {
                    let license = VERSIONS
                        .iter()
                        .find(|(v, _)| *v == version.to_string())
                        .map(|(_, l)| (*l).to_string());
                    Ok(Some(CratesioVersionMeta {
                        cksum: sha256::digest(CRATE),
                        license,
                    }))
                });
            let filters = FilterChain::cratesio(&CratesioFilter {
                license_enabled: true,
                license_allowed: vec!["MIT".to_string()],
                ..CratesioFilter::default()
            })
            .unwrap();
            let worker = worker(db, &proxy, filters);

            predownload_crates(&metadata(), &worker.args).await;

            assert_eq!(vec!["1.0.0"], *upstream.downloads.lock().unwrap());
            assert!(is_cached(&worker, "1.0.0").await);
            assert!(!is_cached(&worker, "1.1.0").await);
        }

        #[tokio::test]
        async fn predownload_without_filters_downloads_all_versions() {
            let upstream = Arc::new(Upstream::default());
            let proxy = fake_cratesio(upstream.clone()).await;
            // No license lookup is expected, the mock panics on unexpected calls
            let worker = worker(MockDb::new(), &proxy, FilterChain::default());

            predownload_crates(&metadata(), &worker.args).await;

            assert!(is_cached(&worker, "1.0.0").await);
            assert!(is_cached(&worker, "1.1.0").await);
        }

        #[tokio::test]
        async fn insert_message_syncs_the_index_it_carries() {
            let upstream = Arc::new(Upstream::default());
            let proxy = fake_cratesio(upstream.clone()).await;
            let (db, stored) = recording_db();
            let worker = worker(db, &proxy, FilterChain::default());

            worker
                .sender
                .send(CratesioPrefetchMsg::Insert(InsertData {
                    name: upstream_name(),
                    etag: Some("etag".to_string()),
                    last_modified: None,
                    data: index(),
                }))
                .unwrap();
            let synced = handle_cratesio_prefetch_msg(&worker.args).await;

            assert_eq!(2, synced.unwrap().len());
            // The index came with the message, only the API is called
            assert_eq!(0, upstream.index_calls.load(Ordering::SeqCst));
            assert_eq!(1, upstream.api_calls.load(Ordering::SeqCst));
            let stored = stored.lock().unwrap();
            assert_eq!(Some(&"MIT".to_string()), stored[0].get("1.0.0"));
            assert_eq!(Some(&"GPL-3.0-only".to_string()), stored[0].get("1.1.0"));
            assert!(!stored[0].values().any(|l| l == UNKNOWN_LICENSE));
        }

        #[tokio::test]
        async fn update_message_refreshes_once_within_the_cache_timeout() {
            let upstream = Arc::new(Upstream::default());
            let proxy = fake_cratesio(upstream.clone()).await;
            let (db, stored) = recording_db();
            let worker = worker(db, &proxy, FilterChain::default());
            let update = || {
                CratesioPrefetchMsg::Update(UpdateData {
                    name: upstream_name(),
                    etag: Some("old-etag".to_string()),
                    last_modified: None,
                })
            };

            worker.sender.send(update()).unwrap();
            let first = handle_cratesio_prefetch_msg(&worker.args).await;
            worker.sender.send(update()).unwrap();
            let second = handle_cratesio_prefetch_msg(&worker.args).await;

            assert_eq!(2, first.unwrap().len());
            assert!(second.is_none());
            assert_eq!(1, upstream.index_calls.load(Ordering::SeqCst));
            assert_eq!(1, stored.lock().unwrap().len());
        }
    }

    fn update_msg(name: &str) -> CratesioPrefetchMsg {
        CratesioPrefetchMsg::Update(UpdateData {
            name: OriginalName::from_unchecked(name.to_string()),
            etag: None,
            last_modified: None,
        })
    }

    #[tokio::test]
    async fn update_round_queues_all_cached_crates() {
        let mut db = MockDb::new();
        db.expect_get_cratesio_index_update_list()
            .returning(|| Ok(vec![update_msg("a"), update_msg("b")]));
        let (sender, receiver) = flume::unbounded();

        let next = queue_update_round(&db, &sender).await;

        assert_eq!(UPDATE_INTERVAL_SECS, next);
        assert_eq!(2, receiver.len());
    }

    #[tokio::test]
    async fn update_round_is_postponed_while_the_previous_one_is_queued() {
        let mut db = MockDb::new();
        db.expect_get_cratesio_index_update_list().never();
        let (sender, receiver) = flume::unbounded();
        sender.send(update_msg("still-queued")).unwrap();

        let next = queue_update_round(&db, &sender).await;

        assert_eq!(UPDATE_LIST_RETRY_SECS, next);
        assert_eq!(1, receiver.len());
    }
}
