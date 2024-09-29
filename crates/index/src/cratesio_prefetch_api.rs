use super::config_json::ConfigJson;
use appstate::{CratesIoPrefetchSenderState, DbState, SettingsState};
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::Json;
use common::cratesio_prefetch_msg::{CratesioPrefetchMsg, InsertData, UpdateData};
use common::index_metadata::IndexMetadata;
use common::normalized_name::NormalizedName;
use common::original_name::OriginalName;
use common::prefetch::Prefetch;
use db::provider::PrefetchState;
use db::{ConString, Database, DbProvider};
use hyper::StatusCode;
use moka::future::Cache;
use reqwest::{Client, ClientBuilder, Url};
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, trace, warn};

static UPDATE_INTERVAL_SECS: u64 = 60 * 120; // 2h background update interval
static UPDATE_CACHE_TIMEOUT_SECS: u64 = 60 * 30; // 30 min cache timeout
static CLIENT: std::sync::LazyLock<Client> = std::sync::LazyLock::new(|| {
    let mut headers = reqwest::header::HeaderMap::new();
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

pub async fn init_cratesio_prefetch_thread(
    con_string: ConString,
    sender: flume::Sender<CratesioPrefetchMsg>,
    recv: flume::Receiver<CratesioPrefetchMsg>,
    num_threads: usize,
    max_con: u32,
) {
    // Threads that takes messages to update the crates.io index
    let db = Arc::new(
        Database::new(&con_string, max_con)
            .await
            .expect("Failed to create database connection for crates.io prefetch thread"),
    );

    let cache = Cache::builder()
        .time_to_live(Duration::from_secs(UPDATE_CACHE_TIMEOUT_SECS))
        .build();

    for _ in 0..num_threads {
        let recv2 = recv.clone();
        let db2 = db.clone();
        let cache2 = cache.clone();

        tokio::spawn(async move {
            cratesio_prefetch_thread(db2, cache2, recv2).await;
        });
    }

    // Thread that periodically checks if the crates.io index needs to be updated.
    // It sends an update message to the thread above which then updates the index.
    tokio::spawn(async move {
        let db = Database::new(&con_string, max_con)
            .await
            .expect("Failed to create database connection for crates.io update thread");
        background_update_thread(db, sender).await;
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
        "Prefetching {} from crates.io cache: Etag {:?} - LM {:?}",
        name,
        if_none_match,
        if_modified_since
    );

    let prefetch_state = db
        .is_cratesio_cache_up_to_date(
            &name.to_normalized(),
            if_none_match.clone(),
            if_modified_since.clone(),
        )
        .await
        .map_err(|e| {
            error!(
                "Could not check if cache is up to date for {}. Error {}",
                name, e
            );
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    match prefetch_state {
        PrefetchState::NeedsUpdate(p) => {
            background_update(name.clone(), sender, if_modified_since, if_none_match);
            trace!("Prefetching {} from crates.io cache: Needs Update", name);
            Ok(p)
        }
        PrefetchState::UpToDate => {
            background_update(name.clone(), sender, if_modified_since, if_none_match);
            trace!("Prefetching {} from crates.io cache: Up to Date", name);
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
        error!("Could not send update message: {}", e);
    }
}

async fn background_update_thread(db: impl DbProvider, sender: flume::Sender<CratesioPrefetchMsg>) {
    loop {
        let crates = match db.get_cratesio_index_update_list().await {
            Ok(crates) => crates,
            Err(e) => {
                error!("Could not get crates.io index update list: {}", e);
                continue;
            }
        };

        for c in crates {
            if let Err(e) = sender.send(c) {
                error!("Could not send update message: {}", e)
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(UPDATE_INTERVAL_SECS)).await;
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

async fn cratesio_prefetch_thread(
    db: Arc<impl DbProvider>,
    cache: Cache<OriginalName, String>,
    channel: flume::Receiver<CratesioPrefetchMsg>,
) {
    loop {
        if let Some((name, metadata, desc, etag, last_modified)) =
            get_insert_data(&cache, &channel).await
        {
            trace!("Update crates.io prefetch data for {}", name);
            if let Err(e) = db
                .add_cratesio_prefetch_data(
                    &name,
                    &etag.unwrap_or_default(),
                    &last_modified.unwrap_or_default(),
                    desc,
                    &metadata,
                )
                .await
            {
                error!(
                    "Could not insert prefetch data from crates.io into database for {}: {}",
                    name, e
                );
            }
        }
    }
}

async fn convert_index_data(
    name: &OriginalName,
    data: String,
) -> Option<(Vec<IndexMetadata>, Option<String>)> {
    let metadata: Result<Vec<IndexMetadata>, serde_json::Error> = data
        .lines()
        .map(serde_json::from_str::<IndexMetadata>)
        .collect();

    match metadata {
        Ok(m) => {
            let desc = fetch_cratesio_description(name).await.unwrap_or_else(|e| {
                error!(
                    "Could not fetch description for from crates.io {}: {:?}",
                    name, e
                );
                None
            });

            Some((m, desc))
        }
        Err(e) => {
            error!(
                "Could not parse prefetch data from crates.io for {}: {}",
                name, e
            );
            None
        }
    }
}

async fn get_insert_data(
    cache: &Cache<OriginalName, String>,
    channel: &flume::Receiver<CratesioPrefetchMsg>,
) -> Option<(
    OriginalName,
    Vec<IndexMetadata>,
    Option<String>,
    Option<String>,
    Option<String>,
)> {
    match channel.recv_async().await {
        Ok(CratesioPrefetchMsg::Insert(msg)) => {
            trace!("Inserting prefetch data from crates.io for {}", msg.name);
            let date = chrono::Utc::now().to_rfc3339();
            cache.insert(msg.name.clone(), date).await;
            convert_index_data(&msg.name, msg.data)
                .await
                .map(|(m, d)| (msg.name.clone(), m, d, msg.etag, msg.last_modified))
        }
        Ok(CratesioPrefetchMsg::Update(msg)) => {
            trace!("Updating prefetch data for {}", msg.name);
            if let Some(date) = cache.get(&msg.name).await {
                trace!("No update needed for {}. Last update: {}", msg.name, date);
                None
            } else {
                cache
                    .insert(msg.name.clone(), chrono::Utc::now().to_rfc3339())
                    .await;
                fetch_index_data(msg.name, msg.etag, msg.last_modified).await
            }
        }
        Err(e) => {
            error!("Could not receive prefetch message: {}", e);
            None
        }
    }
}

async fn fetch_index_data(
    name: OriginalName,
    etag: Option<String>,
    last_modified: Option<String>,
) -> Option<(
    OriginalName,
    Vec<IndexMetadata>,
    Option<String>,
    Option<String>,
    Option<String>,
)> {
    let url = match Url::parse("https://index.crates.io/")
        .unwrap()
        .join(&crate_sub_path(&name.to_normalized()))
    {
        Ok(url) => url,
        Err(e) => {
            error!("Could not parse crates.io url for {}: {}", name, e);
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
                warn!(
                    "Retry {}/{} - Could not fetch index from crates.io for {}: {}",
                    i + 1,
                    max_retries,
                    name,
                    e
                );
                i += 1;
            }
        };
        if i >= max_retries {
            error!(
                "Could not fetch index from crates.io for {} after 3 tries",
                name
            );
            break None;
        }
    };

    if let Some(r) = r {
        match r.status() {
            reqwest::StatusCode::NOT_MODIFIED => {
                trace!("Index not-modified for {}", name);
                None
            }
            reqwest::StatusCode::NOT_FOUND => {
                trace!("Index not found for {}", name);
                None
            }
            reqwest::StatusCode::OK => {
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
                        error!("Could not read index from crates.io for {}: {}", name, e);
                        return None;
                    }
                };

                convert_index_data(&name, data)
                    .await
                    .map(|(m, d)| (name, m, d, etag, last_modified))
            }
            s => {
                error!("Unexpected status code from crates.io for {}: {}", name, s);
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
    let time = std::time::Instant::now();

    let url = Url::parse("https://index.crates.io/")
        .unwrap()
        .join(&crate_sub_path(&name.to_normalized()))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let response = CLIENT.get(url).send().await;

    debug!(
        "Fetching prefetch data from crates.io for {} took {:?}",
        name,
        time.elapsed()
    );

    match response {
        Ok(r) => {
            match r.status() {
                status @ (reqwest::StatusCode::NOT_FOUND
                | reqwest::StatusCode::GONE
                | reqwest::StatusCode::UNAVAILABLE_FOR_LEGAL_REASONS) => {
                    debug!("Crate: '{name}' not available on crates.io ({status})");
                    Err(status)
                }

                reqwest::StatusCode::OK => {
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
                            error!("Could not send prefetch message: {}", e);
                            StatusCode::INTERNAL_SERVER_ERROR
                        })?;

                    Ok(prefetch)
                }
                s => {
                    error!("Unexpected status code from crates.io for {}: {}", name, s);
                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        }
        Err(e) => {
            error!("Error fetching prefetch data from crates.io: {}", e);
            Err(StatusCode::NOT_FOUND)
        }
    }
}

fn crate_sub_path(name: &NormalizedName) -> String {
    match name.len() {
        1 => format!("1/{}", name),
        2 => format!("2/{}", name),
        3 => {
            let first_char = &name[0..1];
            format!("3/{}/{}", first_char, name)
        }
        _ => {
            let first_two = &name[0..2];
            let second_two = &name[2..4];
            format!("{}/{}/{}", first_two, second_two, name)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_json::ConfigJson;
    use appstate::AppStateData;
    use axum::{
        body::Body,
        http::{header, Request},
        routing::get,
        Router,
    };
    use db::mock::MockDb;
    use http_body_util::BodyExt;
    use settings::{Protocol, Settings};
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
            .await
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
            .await
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
            .await
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
                &Protocol::Http,
                "test.api.com",
                1234,
                "cratesio",
                false,
                false
            ),
            actual
        );
    }

    async fn app() -> Router {
        let settings = Settings {
            origin: settings::Origin {
                protocol: Protocol::Http,
                hostname: String::from("test.api.com"),
                port: 1234,
            },
            ..Settings::default()
        };

        let mut mock_db = MockDb::new();

        mock_db
            .expect_add_cratesio_prefetch_data()
            .returning(move |_, _, _, _, _| {
                Ok(Prefetch {
                    data: vec![0x1, 0x2, 0x3],
                    etag: String::from("etag"),
                    last_modified: String::from("date"),
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
            .route("/:_/:_/:name", get(prefetch_cratesio))
            .route("/:_/:name", get(prefetch_len2_cratesio));

        let state = AppStateData {
            db: Arc::new(mock_db),
            settings: Arc::new(settings),
            cratesio_prefetch_sender: sender,
            ..appstate::test_state().await
        };

        Router::new()
            .nest("/api/v1/cratesio", cratesio_prefetch)
            .with_state(state)
    }
}
