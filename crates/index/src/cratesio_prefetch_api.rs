use super::config_json::ConfigJson;
use auth::auth_req_token::AuthReqToken;
use common::cratesio_prefetch_msg::{CratesioPrefetchMsg, InsertData, UpdateData};
use common::index_metadata::IndexMetadata;
use common::normalized_name::NormalizedName;
use common::original_name::OriginalName;
use common::prefetch::{Headers, Prefetch};
use db::provider::PrefetchState;
use db::DbProvider;
use moka::future::Cache;
use reqwest::{Client, StatusCode, Url};
use rocket::http::Status;
use rocket::response::status;
use rocket::State;
use serde::Deserialize;
use settings::Settings;
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, trace, warn};
/// The API for the crates.io sparse index
/// https://github.com/rust-lang/cargo/blob/c015bfd1efb04c0bf60df9bfef2b8b3b6633310b/src/doc/src/reference/registry-index.md#index-protocols

static UPDATE_INTERVAL_SECS: u64 = 60 * 120; // 2h background update interval
static UPDATE_CACHE_TIMEOUT_SECS: u64 = 60 * 30; // 30 min cache timeout

// #[get("/config.json")]
pub async fn config_cratesio(
    settings: &State<Settings>,
    auth_req_token: AuthReqToken,
) -> ConfigJson {
    _ = auth_req_token;
    ConfigJson::from((settings.inner(), "cratesio"))
}

// #[get("/<_>/<_>/<name>", rank = 1)]
pub async fn prefetch_cratesio(
    name: OriginalName,
    headers: Headers,
    auth_req_token: AuthReqToken,
    db: &State<Box<dyn DbProvider>>,
    sender: &State<Arc<flume::Sender<CratesioPrefetchMsg>>>,
) -> Result<Prefetch, status::Custom<&'static str>> {
    _ = auth_req_token;
    internal_prefetch_cratesio(name, headers, db, sender).await
}

// Example: The package "h2" is under "/2/h2" in the index and thus no second path element exists.
// #[get("/<_>/<name>", rank = 1)]
pub async fn prefetch_len2_cratesio(
    name: OriginalName,
    headers: Headers,
    auth_req_token: AuthReqToken,
    db: &State<Box<dyn DbProvider>>,
    sender: &State<Arc<flume::Sender<CratesioPrefetchMsg>>>,
) -> Result<Prefetch, status::Custom<&'static str>> {
    _ = auth_req_token;
    internal_prefetch_cratesio(name, headers, db, sender).await
}

async fn internal_prefetch_cratesio(
    name: OriginalName,
    headers: Headers,
    db: &State<Box<dyn DbProvider>>,
    sender: &State<Arc<flume::Sender<CratesioPrefetchMsg>>>,
) -> Result<Prefetch, status::Custom<&'static str>> {
    let if_modified_since = headers.if_modified_since;
    let if_none_match = headers.if_none_match;

    trace!(
        "Prefetching {} from crates.io cache: Etag {} - LM {}",
        name,
        if_none_match.clone().unwrap_or_default(),
        if_modified_since.clone().unwrap_or_default()
    );

    match db
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
            status::Custom(
                Status::InternalServerError,
                "Could not check if cache is up to date",
            )
        })? {
        PrefetchState::NeedsUpdate(p) => {
            background_update(name.clone(), sender, if_modified_since, if_none_match);
            trace!("Prefetching {} from crates.io cache: Needs Update", name);
            Ok(p)
        }
        PrefetchState::UpToDate => {
            background_update(name.clone(), sender, if_modified_since, if_none_match);
            trace!("Prefetching {} from crates.io cache: Up to Date", name);
            Err(status::Custom(Status::NotModified, "Index up-to-date"))
        }
        PrefetchState::NotFound => Ok(fetch_cratesio_prefetch(name, sender).await?),
    }
}

fn background_update(
    name: OriginalName,
    sender: &State<Arc<flume::Sender<CratesioPrefetchMsg>>>,
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

pub async fn background_update_thread(
    db: impl DbProvider,
    sender: Arc<flume::Sender<CratesioPrefetchMsg>>,
) {
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

        rocket::tokio::time::sleep(rocket::tokio::time::Duration::from_secs(
            UPDATE_INTERVAL_SECS,
        ))
        .await;
    }
}

async fn fetch_cratesio_description(
    name: &str,
) -> Result<Option<String>, status::Custom<&'static str>> {
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

    let response = Client::new()
        .get(url)
        .header("User-Agent", "kellnr.io/kellnr")
        .send()
        .await
        .map_err(|_| {
            status::Custom(
                Status::ServiceUnavailable,
                "Could not fetch description from crates.io",
            )
        })?;

    let desc = response.json::<MinimalCrate>().await.map_err(|_| {
        status::Custom(
            Status::InternalServerError,
            "Could not read description from crates.io",
        )
    })?;
    Ok(desc.krate.description)
}

pub async fn cratesio_prefetch_thread(
    db: Arc<impl DbProvider>,
    channel: Arc<flume::Receiver<CratesioPrefetchMsg>>,
) {
    let cache: Cache<String, String> = Cache::builder()
        .time_to_live(Duration::from_secs(UPDATE_CACHE_TIMEOUT_SECS))
        .build();

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
    cache: &Cache<String, String>,
    channel: &Arc<flume::Receiver<CratesioPrefetchMsg>>,
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
            cache.insert(msg.name.to_string(), date).await;
            convert_index_data(&msg.name, msg.data)
                .await
                .map(|(m, d)| (msg.name.clone(), m, d, msg.etag, msg.last_modified))
        }
        Ok(CratesioPrefetchMsg::Update(msg)) => {
            trace!("Updating prefetch data for {}", msg.name);
            if let Some(date) = cache.get(msg.name.deref()).await {
                trace!("No update needed for {}. Last update: {}", msg.name, date);
                None
            } else {
                cache
                    .insert(msg.name.to_string(), chrono::Utc::now().to_rfc3339())
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
    let r = loop {
        let mut i = 0;
        match Client::new()
            .get(url.clone())
            .header("User-Agent", "kellnr.io/kellnr")
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
        if i > max_retries {
            error!(
                "Could not fetch index from crates.io for {} after 3 tries",
                name
            );
            break None;
        }
    };

    if let Some(r) = r {
        match r.status() {
            StatusCode::NOT_MODIFIED => {
                trace!("Index not-modified for {}", name);
                None
            }
            StatusCode::NOT_FOUND => {
                trace!("Index not found for {}", name);
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
    sender: &State<Arc<flume::Sender<CratesioPrefetchMsg>>>,
) -> Result<Prefetch, status::Custom<&'static str>> {
    let url = Url::parse("https://index.crates.io/")
        .unwrap()
        .join(&crate_sub_path(&name.to_normalized()))
        .map_err(|_| status::Custom(Status::InternalServerError, "Failed to parse URL"))?;

    let response = Client::new()
        .get(url)
        .header("User-Agent", "kellnr.io/kellnr")
        .send()
        .await;

    match response {
        Ok(r) => {
            let headers = r.headers();
            let etag = headers
                .get("ETag")
                .map(|h| h.to_str().unwrap_or_default().to_string());
            let last_modified = headers
                .get("Last-Modified")
                .map(|h| h.to_str().unwrap_or_default().to_string());

            let data = r.text().await.map_err(|_| {
                status::Custom(Status::InternalServerError, "Could not read response body")
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
                    error!("Could not send prefetch message: {}", e);
                    status::Custom(
                        Status::InternalServerError,
                        "Could not send prefetch message",
                    )
                })?;

            Ok(prefetch)
        }
        Err(e) => {
            error!("Error fetching prefetch data from crates.io: {}", e);
            Err(status::Custom(Status::NotFound, "Index not found"))
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
    use db::mock::MockDb;
    use settings::{Protocol, Settings};
    use std::mem;
    use axum::Router;

    #[async_test]
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

    #[async_test]
    async fn fetch_cratesio_description_not_existent_crate() {
        let desc = fetch_cratesio_description("does_not_exists123").await;
        assert_eq!(
            Err(Custom(
                Status { code: 500 },
                "Could not read description from crates.io"
            )),
            desc
        );
    }

    #[test]
    fn fetch_cratesio_prefetch_works() {
        let client = client();
        let req = client
            .get("/api/v1/cratesio/ro/ck/rocket")
            .header(Header::new("If-Modified-Since", "date"))
            .header(Header::new("ETag", "etag"));

        let result = req.dispatch();
        let status = result.status();
        let prefetch = result.into_bytes().unwrap();

        assert_eq!(status, Status::Ok);
        assert!(prefetch.len() > 500);
    }

    #[test]
    fn config_returns_config_json() {
        let client = client();
        let req = client.get("/api/v1/cratesio/config.json");
        let result = req.dispatch();
        let result_msg = result.into_string().unwrap();
        let actual = serde_json::from_str::<ConfigJson>(&result_msg).unwrap();

        assert_eq!(
            ConfigJson::new(&Protocol::Http, "test.api.com", 1234, "cratesio", false),
            actual
        );
    }

    async fn app() -> Router {
        let settings = Settings {
            api_address: String::from("test.api.com"),
            api_port: 8000,
            api_port_proxy: 1234,
            ..Settings::new().unwrap()
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
        let sender = Arc::new(sender);
        // Make receiver undroppable, else the sender will fail, as the receiver is dropped when
        // this function goes out of scope
        mem::forget(receiver);

        
        let cratesio_prefetch = Router::new()
        .route("/config.json", get(cratesio_prefetch_api::config_cratesio))
        .route("/:_/:_/:name", get(cratesio_prefetch_api::prefetch_cratesio))
        .route("/:_/:name", get(cratesio_prefetch_api::prefetch_len2_cratesio));


        Router::new()
            .nest("/api/v1/cratesio", cratesio_prefetch)
            .withState(appstate)
    }

    fn client() -> Client {
        let settings = Settings {
            api_address: String::from("test.api.com"),
            api_port: 8000,
            api_port_proxy: 1234,
            ..Settings::new().unwrap()
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

        let db = Box::new(mock_db) as Box<dyn DbProvider>;

        let rocket_conf = Config {
            secret_key: SecretKey::generate().expect("Unable to create a secret key."),
            ..Config::default()
        };

        let (sender, receiver) = flume::unbounded::<CratesioPrefetchMsg>();
        let sender = Arc::new(sender);
        // Make receiver undroppable, else the sender will fail, as the receiver is dropped when
        // this function goes out of scope
        mem::forget(receiver);

        let rocket = rocket::custom(rocket_conf)
            .mount(
                "/api/v1/cratesio",
                routes![config_cratesio, prefetch_cratesio, prefetch_len2_cratesio],
            )
            .manage(db)
            .manage(sender)
            .manage(settings);

        Client::tracked(rocket).unwrap()
    }
}
