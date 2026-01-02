use super::config_json::ConfigJson;
use kellnr_appstate::{CratesIoPrefetchSenderState, DbState, SettingsState};
use axum::Json;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use kellnr_common::cratesio_downloader::download_crate;
use kellnr_common::cratesio_prefetch_msg::{CratesioPrefetchMsg, InsertData, UpdateData};
use kellnr_common::index_metadata::IndexMetadata;
use kellnr_common::normalized_name::NormalizedName;
use kellnr_common::original_name::OriginalName;
use kellnr_common::prefetch::Prefetch;
use kellnr_common::version::Version;
use kellnr_db::DbProvider;
use kellnr_db::provider::PrefetchState;
use hyper::StatusCode;
use moka::future::Cache;
use reqwest::{Client, ClientBuilder, Url};
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;
use kellnr_storage::cratesio_crate_storage::CratesIoCrateStorage;
use tracing::{debug, error, trace, warn};

pub static UPDATE_INTERVAL_SECS: u64 = 60 * 120; // 2h background update interval
pub static UPDATE_CACHE_TIMEOUT_SECS: u64 = 60 * 30; // 30 min cache timeout
static CLIENT: std::sync::LazyLock<Client> = std::sync::LazyLock::new(|| {
    let mut headers = HeaderMap::new();
    headers.insert(
        reqwest::header::USER_AGENT,
        reqwest::header::HeaderValue::from_static("kellnr.io/kellnr"),
    );
    ClientBuilder::new()
        .gzip(true)
        .default_headers(headers)
        .build()
        .unwrap()
});

#[allow(clippy::unused_async)] // part of the router
pub async fn config_cratesio(State(settings): SettingsState) -> Json<ConfigJson> {
    Json(ConfigJson::from((&(*settings), "cratesio", false)))
}

pub async fn prefetch_cratesio(
    Path((_a, _b, name)): Path<(String, String, OriginalName)>,
    headers: HeaderMap,
    State(db): DbState,
    State(sender): CratesIoPrefetchSenderState,
) -> Result<Prefetch, StatusCode> {
    internal_prefetch_cratesio(name, headers, &db, &sender).await
}

pub async fn prefetch_len2_cratesio(
    Path((_a, name)): Path<(String, OriginalName)>,
    headers: HeaderMap,
    State(db): DbState,
    State(sender): CratesIoPrefetchSenderState,
) -> Result<Prefetch, StatusCode> {
    internal_prefetch_cratesio(name, headers, &db, &sender).await
}

pub fn init_cratesio_prefetch_thread(
    sender: flume::Sender<CratesioPrefetchMsg>,
    num_threads: usize,
    args: CratesIoPrefetchArgs,
) {
    // Threads that takes messages to update the crates.io index
    for _ in 0..num_threads {
        let args = CratesIoPrefetchArgs {
            db: args.db.clone(),
            cache: args.cache.clone(),
            recv: args.recv.clone(),
            download_on_update: args.download_on_update,
            storage: args.storage.clone(),
        };

        tokio::spawn(async move {
            cratesio_prefetch_thread(args).await;
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
        .is_cratesio_cache_up_to_date(
            &name.to_normalized(),
            if_none_match.clone(),
            if_modified_since.clone(),
        )
        .await
        .map_err(|e| {
            error!("Could not check if cache is up to date for {name}. Error {e}",);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    match prefetch_state {
        PrefetchState::NeedsUpdate(p) => {
            background_update(name.clone(), sender, if_modified_since, if_none_match);
            trace!("Prefetching {name} from crates.io cache: Needs Update");
            Ok(p)
        }
        PrefetchState::UpToDate => {
            background_update(name.clone(), sender, if_modified_since, if_none_match);
            trace!("Prefetching {name} from crates.io cache: Up to Date");
            Err(StatusCode::NOT_MODIFIED)
        }
        PrefetchState::NotFound => Ok(fetch_cratesio_prefetch(name, sender).await?),
    }
}

fn background_update(
    name: OriginalName,
    sender: &flume::Sender<CratesioPrefetchMsg>,
    if_modified_since: Option<String>,
    if_none_match: Option<String>,
) {
    if let Err(e) = sender.send(CratesioPrefetchMsg::Update(UpdateData {
        name,
        etag: if_none_match,
        last_modified: if_modified_since,
    })) {
        error!("Could not send update message: {e}");
    }
}

async fn background_update_thread(
    db: Arc<dyn DbProvider>,
    sender: flume::Sender<CratesioPrefetchMsg>,
) {
    loop {
        let crates = match db.get_cratesio_index_update_list().await {
            Ok(crates) => crates,
            Err(e) => {
                error!("Could not get crates.io index update list: {e}");
                continue;
            }
        };

        for c in crates {
            if let Err(e) = sender.send(c) {
                error!("Could not send update message: {e}");
            }
        }

        tokio::time::sleep(Duration::from_secs(UPDATE_INTERVAL_SECS)).await;
    }
}

async fn fetch_cratesio_description(name: &str) -> Result<Option<String>, StatusCode> {
    #[derive(Deserialize)]
    struct Krate {
        description: Option<String>,
    }
    #[derive(Deserialize)]
    struct MinimalCrate {
        #[serde(rename = "crate")]
        krate: Krate,
    }

    let url = Url::parse("https://crates.io/api/v1/crates/")
        .unwrap()
        .join(name)
        .unwrap();

    let response = CLIENT
        .get(url)
        .send()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let desc = response
        .json::<MinimalCrate>()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(desc.krate.description)
}

pub struct CratesIoPrefetchArgs {
    pub db: Arc<dyn DbProvider>,
    pub cache: Cache<OriginalName, String>,
    pub recv: flume::Receiver<CratesioPrefetchMsg>,
    pub download_on_update: bool,
    pub storage: Arc<CratesIoCrateStorage>,
}

async fn cratesio_prefetch_thread(args: CratesIoPrefetchArgs) -> ! {
    loop {
        match handle_cratesio_prefetch_msg(&args.cache, &args.recv, &args.db).await {
            UpdateNeeded::Update(data) => {
                if let Err(e) = args
                    .db
                    .add_cratesio_prefetch_data(
                        &data.name,
                        &data.etag.unwrap_or_default(),
                        &data.last_modified.unwrap_or_default(),
                        data.description,
                        &data.metadata,
                    )
                    .await
                {
                    error!(
                        "Could not insert prefetch data from crates.io into database for {}: {e}",
                        data.name
                    );
                }
                // If the download_on_update flag is set, do not only update the prefetch data in
                // the database, but also download the crates.
                else if args.download_on_update {
                    for idx in &data.metadata {
                        predownload_crate(idx, &args).await;
                    }
                }
            }
            UpdateNeeded::NoUpdate => {
                trace!("No update needed for crates.io prefetch data");
            }
        }
    }
}

async fn predownload_crate(idx: &IndexMetadata, args: &CratesIoPrefetchArgs) {
    let name = OriginalName::from_unchecked(idx.name.clone());
    let version = Version::from_unchecked_str(&idx.vers);

    match args.storage.exists(&name, &version).await {
        Ok(true) => {
            trace!(
                "Crate {} version {} already exists in storage, skipping download",
                idx.name, idx.vers
            );
        }
        Ok(false) => {
            trace!("Downloading version {} for crate {}", idx.vers, idx.name);
            match download_crate(&idx.name, &idx.vers).await {
                Ok(crate_data) => {
                    if let Err(e) = args
                        .storage
                        .put(
                            &OriginalName::from_unchecked(idx.name.clone()),
                            &Version::from_unchecked_str(&idx.vers),
                            crate_data,
                        )
                        .await
                    {
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

#[derive(Debug, Clone, Default)]
struct MetadataDescription {
    metadata: Vec<IndexMetadata>,
    description: Option<String>,
}

async fn convert_index_data(name: &str, data: &str) -> MetadataDescription {
    let metadata: Result<Vec<IndexMetadata>, serde_json::Error> = data
        .lines()
        .map(serde_json::from_str::<IndexMetadata>)
        .collect();

    match metadata {
        Ok(m) => {
            let desc = fetch_cratesio_description(name).await.unwrap_or_else(|e| {
                error!("Could not fetch description for from crates.io {name}: {e:?}",);
                None
            });

            MetadataDescription {
                metadata: m,
                description: desc,
            }
        }
        Err(e) => {
            error!("Could not parse prefetch data from crates.io for {name}: {e}",);
            MetadataDescription::default()
        }
    }
}

struct PrefetchData {
    name: OriginalName,
    metadata: Vec<IndexMetadata>,
    description: Option<String>,
    etag: Option<String>,
    last_modified: Option<String>,
}

impl PrefetchData {
    fn new(
        name: OriginalName,
        etag: Option<String>,
        last_modified: Option<String>,
        metadata_desc: MetadataDescription,
    ) -> Self {
        Self {
            name,
            metadata: metadata_desc.metadata,
            description: metadata_desc.description,
            etag,
            last_modified,
        }
    }
}

enum UpdateNeeded {
    Update(PrefetchData),
    NoUpdate,
}

impl From<PrefetchData> for UpdateNeeded {
    fn from(value: PrefetchData) -> Self {
        Self::Update(value)
    }
}

impl From<Option<PrefetchData>> for UpdateNeeded {
    fn from(value: Option<PrefetchData>) -> Self {
        match value {
            Some(data) => Self::Update(data),
            None => Self::NoUpdate,
        }
    }
}

async fn handle_cratesio_prefetch_msg(
    cache: &Cache<OriginalName, String>,
    channel: &flume::Receiver<CratesioPrefetchMsg>,
    db: &Arc<dyn DbProvider>,
) -> UpdateNeeded {
    match channel.recv_async().await {
        Ok(CratesioPrefetchMsg::Insert(msg)) => {
            trace!("Inserting prefetch data from crates.io for {}", msg.name);
            let date = chrono::Utc::now().to_rfc3339();
            cache.insert(msg.name.clone(), date).await;
            let metadata_desc = convert_index_data(&msg.name, &msg.data).await;
            PrefetchData::new(msg.name, msg.etag, msg.last_modified, metadata_desc).into()
        }
        Ok(CratesioPrefetchMsg::Update(msg)) => {
            trace!("Updating prefetch data for {}", msg.name);
            if let Some(date) = cache.get(&msg.name).await {
                trace!("No update needed for {}. Last update: {date}", msg.name);
                UpdateNeeded::NoUpdate
            } else {
                cache
                    .insert(msg.name.clone(), chrono::Utc::now().to_rfc3339())
                    .await;
                fetch_index_data(msg).await.into()
            }
        }
        Ok(CratesioPrefetchMsg::IncDownloadCnt(msg)) => {
            trace!(
                "Incrementing download count for {} {}",
                msg.name, msg.version
            );
            db.increase_cached_download_counter(&msg.name, &msg.version)
                .await
                .unwrap_or_else(|e| warn!("Failed to increase download counter: {e}"));
            UpdateNeeded::NoUpdate
        }
        Err(e) => {
            error!("Could not receive prefetch message: {e}");
            UpdateNeeded::NoUpdate
        }
    }
}

async fn fetch_index_data(msg: UpdateData) -> Option<PrefetchData> {
    let name = &msg.name;
    let etag = &msg.etag;
    let last_modified = &msg.last_modified;

    let url = match Url::parse("https://index.crates.io/")
        .unwrap()
        .join(&crate_sub_path(&name.to_normalized()))
    {
        Ok(url) => url,
        Err(e) => {
            error!("Could not parse crates.io url for {name}: {e}");
            return None;
        }
    };

    let max_retries = 3;
    let mut i = 0;
    let r = loop {
        match CLIENT
            .get(url.clone())
            .header("If-None-Match", etag.clone().unwrap_or_default())
            .header(
                "If-Modified-Since",
                last_modified.clone().unwrap_or_default(),
            )
            .send()
            .await
        {
            Ok(response) => break Some(response),
            Err(e) => {
                i += 1;
                warn!(
                    "Retry {i}/{max_retries} - Could not fetch index from crates.io for {name}: {e}"
                );
            }
        }
        if i >= max_retries {
            error!("Could not fetch index from crates.io for {name} after 3 tries",);
            break None;
        }
    };

    if let Some(r) = r {
        match r.status() {
            StatusCode::NOT_MODIFIED => {
                trace!("Index not-modified for {name}");
                None
            }
            StatusCode::NOT_FOUND => {
                trace!("Index not found for {name}");
                None
            }
            StatusCode::OK => {
                let headers = r.headers();
                let etag = headers
                    .get("ETag")
                    .map(|h| h.to_str().unwrap_or_default().to_string());
                let last_modified = headers
                    .get("Last-Modified")
                    .map(|h| h.to_str().unwrap_or_default().to_string());

                let data = match r.text().await {
                    Ok(data) => data,
                    Err(e) => {
                        error!("Could not read index from crates.io for {name}: {e}");
                        return None;
                    }
                };

                let metadata_desc = convert_index_data(&msg.name, &data).await;
                let prefetch_data = PrefetchData::new(msg.name, etag, last_modified, metadata_desc);
                Some(prefetch_data)
            }
            s => {
                error!("Unexpected status code from crates.io for {name}: {s}");
                None
            }
        }
    } else {
        None
    }
}

async fn fetch_cratesio_prefetch(
    name: OriginalName,
    sender: &flume::Sender<CratesioPrefetchMsg>,
) -> Result<Prefetch, StatusCode> {
    let url = Url::parse("https://index.crates.io/")
        .unwrap()
        .join(&crate_sub_path(&name.to_normalized()))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let response = CLIENT.get(url).send().await;

    match response {
        Ok(r) => {
            match r.status() {
                status @ (StatusCode::NOT_FOUND
                | StatusCode::GONE
                | StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS) => {
                    debug!("Crate: '{name}' not available on crates.io ({status})");
                    Err(status)
                }

                StatusCode::OK => {
                    let headers = r.headers();
                    let etag = headers
                        .get("ETag")
                        .map(|h| h.to_str().unwrap_or_default().to_string());
                    let last_modified = headers
                        .get("Last-Modified")
                        .map(|h| h.to_str().unwrap_or_default().to_string());

                    let data = r
                        .text()
                        .await
                        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

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
                s => {
                    error!("Unexpected status code from crates.io for {name}: {s}");
                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        }
        Err(e) => {
            error!("Error fetching prefetch data from crates.io: {e}");
            Err(StatusCode::NOT_FOUND)
        }
    }
}

fn crate_sub_path(name: &NormalizedName) -> String {
    match name.len() {
        1 => format!("1/{name}"),
        2 => format!("2/{name}"),
        3 => {
            let first_char = &name[0..1];
            format!("3/{first_char}/{name}")
        }
        _ => {
            let first_two = &name[0..2];
            let second_two = &name[2..4];
            format!("{first_two}/{second_two}/{name}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_json::ConfigJson;
    use kellnr_appstate::AppStateData;
    use axum::{
        Router,
        body::Body,
        http::{Request, header},
        routing::get,
    };
    use kellnr_db::mock::MockDb;
    use http_body_util::BodyExt;
    use kellnr_settings::{Protocol, Settings};
    use std::mem;
    use tower::ServiceExt;

    #[tokio::test]
    async fn fetch_cratesio_description_works() {
        let desc = fetch_cratesio_description("rocket").await.unwrap();
        assert_eq!(
            Some(
                "Web framework with a focus on usability, security, extensibility, and speed.\n"
                    .to_string()
            ),
            desc
        );
    }

    #[tokio::test]
    async fn fetch_cratesio_description_not_existent_crate() {
        let desc = fetch_cratesio_description("does_not_exists123").await;
        assert_eq!(Err(StatusCode::INTERNAL_SERVER_ERROR), desc);
    }

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
        let actual = serde_json::from_slice::<ConfigJson>(&result_msg).unwrap();

        assert_eq!(
            ConfigJson::new(
                Protocol::Http,
                "test.api.com",
                1234,
                None,
                "cratesio",
                false,
                false
            ),
            actual
        );
    }

    fn app() -> Router {
        let settings = Settings {
            origin: kellnr_settings::Origin {
                protocol: Protocol::Http,
                hostname: "test.api.com".to_string(),
                port: 1234,
                path: String::new(),
            },
            ..Settings::default()
        };

        let mut mock_db = MockDb::new();

        mock_db
            .expect_add_cratesio_prefetch_data()
            .returning(move |_, _, _, _, _| {
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
            ..kellnr_appstate::test_state()
        };

        Router::new()
            .nest("/api/v1/cratesio", cratesio_prefetch)
            .with_state(state)
    }
}
