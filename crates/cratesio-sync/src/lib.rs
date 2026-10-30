//! Keeps the cached crates.io metadata of a crate in the database up to date:
//! the index (with the checksums) and, from the crates.io API, the description
//! and the license of each version.
//!
//! All writers go through [`CratesioSync`], which runs at most one sync per
//! crate at a time. Callers that arrive while a sync of the same crate is
//! running wait for it instead of starting another one. This lets a download
//! wait until the checksum and license of its version are known.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use futures_util::FutureExt;
use futures_util::future::{BoxFuture, Shared};
use kellnr_common::canonical_name::CanonicalName;
use kellnr_common::index_metadata::IndexMetadata;
use kellnr_common::license::UNKNOWN_LICENSE;
use kellnr_common::original_name::OriginalName;
use kellnr_db::DbProvider;
use kellnr_db::provider::PrefetchState;
use reqwest::{Client, RequestBuilder, Response, StatusCode, Url};
use serde::Deserialize;
use tracing::{error, trace, warn};

/// Tries of a request to crates.io that fails with a transport error, 429 or
/// a server error, with a backoff between them that starts at
/// `RETRY_BASE_DELAY` and doubles.
const MAX_TRIES: u32 = 3;
const RETRY_BASE_DELAY: Duration = Duration::from_millis(500);
/// Upper bound for the wait between tries, also if crates.io asks for a longer
/// one with `Retry-After`.
const MAX_RETRY_DELAY: Duration = Duration::from_secs(5);

/// Where the index data of a sync comes from.
pub enum SyncSource {
    /// The index was already fetched, e.g. to answer cargo. Nothing is done if
    /// the database already holds it.
    Index {
        etag: Option<String>,
        last_modified: Option<String>,
        data: String,
    },
    /// Fetch the index with the stored validators. Nothing is done if it did
    /// not change upstream.
    Refresh {
        etag: Option<String>,
        last_modified: Option<String>,
    },
    /// Fetch the index unconditionally, e.g. because the checksum or license
    /// of a version is missing.
    Fetch,
}

/// The index metadata written by a sync, `None` if nothing was written.
pub type Synced = Option<Arc<[IndexMetadata]>>;

type Job = Shared<BoxFuture<'static, Synced>>;

#[derive(Clone)]
pub struct CratesioSync {
    inner: Arc<Inner>,
}

struct Inner {
    db: Arc<dyn DbProvider>,
    client: Client,
    index_url: Url,
    api_url: Url,
    running: Mutex<HashMap<CanonicalName, Job>>,
}

impl CratesioSync {
    pub fn new(db: Arc<dyn DbProvider>, client: Client, proxy: &kellnr_settings::Proxy) -> Self {
        Self {
            inner: Arc::new(Inner {
                db,
                client,
                index_url: proxy.index.clone(),
                api_url: proxy.api.clone(),
                running: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// Sync the crate, or wait for the sync of it that is already running, in
    /// which case `source` is not used.
    pub async fn sync(&self, name: &OriginalName, source: SyncSource) -> Synced {
        let key = name.to_canonical();
        let job = {
            let mut running = self
                .inner
                .running
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if let Some(job) = running.get(&key) {
                trace!("Waiting for the running crates.io sync of {name}");
                job.clone()
            } else {
                // Spawned, so the sync finishes even if all callers go away.
                let inner = self.inner.clone();
                let name = name.clone();
                let task_key = key.clone();
                let task = tokio::spawn(async move {
                    let _done = RemoveOnDrop {
                        inner: inner.clone(),
                        key: task_key,
                    };
                    inner.run(&name, source).await
                });
                let job = async move { task.await.ok().flatten() }.boxed().shared();
                running.insert(key, job.clone());
                job
            }
        };
        job.await
    }
}

/// Removes a finished sync from the running ones. On drop, so a sync that
/// panics does not block all later syncs of its crate.
struct RemoveOnDrop {
    inner: Arc<Inner>,
    key: CanonicalName,
}

impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        self.inner
            .running
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&self.key);
    }
}

struct IndexData {
    etag: Option<String>,
    last_modified: Option<String>,
    data: String,
}

impl Inner {
    async fn run(&self, name: &OriginalName, source: SyncSource) -> Synced {
        let index = match source {
            SyncSource::Index {
                etag,
                last_modified,
                data,
            } => {
                if self
                    .is_stored(name, etag.clone(), last_modified.clone())
                    .await
                {
                    trace!("crates.io index of {name} is already stored");
                    return None;
                }
                IndexData {
                    etag,
                    last_modified,
                    data,
                }
            }
            SyncSource::Refresh {
                etag,
                last_modified,
            } => self.fetch_index(name, etag, last_modified).await?,
            SyncSource::Fetch => self.fetch_index(name, None, None).await?,
        };

        let metadata = parse_index(name, &index.data)?;
        // An API failure must not keep the index and checksums from being
        // written. The licenses stay unknown and are fetched on the next sync.
        let info = self.fetch_crate_info(name).await.unwrap_or_else(|e| {
            warn!("Could not fetch crate info of {name} from crates.io: {e}");
            CrateInfo::default()
        });

        match self
            .db
            .add_cratesio_prefetch_data(
                name,
                &index.etag.unwrap_or_default(),
                &index.last_modified.unwrap_or_default(),
                info.description,
                &metadata,
                &info.licenses,
            )
            .await
        {
            Ok(_) => Some(metadata.into()),
            Err(e) => {
                error!(
                    "Could not insert prefetch data from crates.io into database for {name}: {e}"
                );
                None
            }
        }
    }

    async fn is_stored(
        &self,
        name: &OriginalName,
        etag: Option<String>,
        last_modified: Option<String>,
    ) -> bool {
        matches!(
            self.db
                .is_cratesio_cache_up_to_date(name, etag, last_modified)
                .await,
            Ok(PrefetchState::UpToDate { .. })
        )
    }

    async fn fetch_index(
        &self,
        name: &OriginalName,
        etag: Option<String>,
        last_modified: Option<String>,
    ) -> Option<IndexData> {
        let url = match self.index_url.join(&crate_sub_path(name)) {
            Ok(url) => url,
            Err(e) => {
                error!("Could not parse crates.io url for {name}: {e}");
                return None;
            }
        };

        let response = send_with_retries(&format!("index of {name}"), || {
            let mut request = self.client.get(url.clone());
            if let Some(etag) = etag.as_deref().filter(|e| !e.is_empty()) {
                request = request.header("If-None-Match", etag);
            }
            if let Some(last_modified) = last_modified.as_deref().filter(|lm| !lm.is_empty()) {
                request = request.header("If-Modified-Since", last_modified);
            }
            request
        })
        .await;
        let response = match response {
            Ok(response) => response,
            Err(e) => {
                error!("Could not fetch index from crates.io for {name}: {e}");
                return None;
            }
        };

        match response.status() {
            StatusCode::NOT_MODIFIED => {
                trace!("Index not-modified for {name}");
                None
            }
            StatusCode::NOT_FOUND => {
                trace!("Index not found for {name}");
                None
            }
            StatusCode::OK => {
                let headers = response.headers();
                let etag = headers
                    .get("ETag")
                    .map(|h| h.to_str().unwrap_or_default().to_string());
                let last_modified = headers
                    .get("Last-Modified")
                    .map(|h| h.to_str().unwrap_or_default().to_string());

                match response.text().await {
                    Ok(data) => Some(IndexData {
                        etag,
                        last_modified,
                        data,
                    }),
                    Err(e) => {
                        error!("Could not read index from crates.io for {name}: {e}");
                        None
                    }
                }
            }
            s => {
                error!("Unexpected status code from crates.io for {name}: {s}");
                None
            }
        }
    }

    /// Description and licenses of the crate from the crates.io API, from the
    /// same request.
    async fn fetch_crate_info(&self, name: &OriginalName) -> Result<CrateInfo, String> {
        #[derive(Deserialize)]
        struct Krate {
            description: Option<String>,
        }
        #[derive(Deserialize)]
        struct ApiVersion {
            num: String,
            license: Option<String>,
        }
        #[derive(Deserialize)]
        struct ApiCrate {
            #[serde(rename = "crate")]
            krate: Krate,
            #[serde(default)]
            versions: Vec<ApiVersion>,
        }

        let url = self.api_url.join(name).map_err(|e| e.to_string())?;
        let response = send_with_retries(&format!("crate info of {name}"), || {
            self.client.get(url.clone())
        })
        .await
        .and_then(Response::error_for_status)
        .map_err(|e| e.to_string())?;
        let api_crate = response
            .json::<ApiCrate>()
            .await
            .map_err(|e| e.to_string())?;

        let licenses = api_crate
            .versions
            .into_iter()
            .map(|v| {
                let license = v
                    .license
                    .filter(|l| !l.trim().is_empty())
                    .unwrap_or_else(|| UNKNOWN_LICENSE.to_string());
                (v.num, license)
            })
            .collect();

        Ok(CrateInfo {
            description: api_crate.krate.description,
            licenses,
        })
    }
}

/// Sends the request built by `request` and retries it with a backoff on
/// transport errors, 429 and server errors. Returns the last result.
async fn send_with_retries(
    what: &str,
    request: impl Fn() -> RequestBuilder,
) -> reqwest::Result<Response> {
    let mut delay = RETRY_BASE_DELAY;
    let mut tries = 1;
    loop {
        let result = request().send().await;
        let (retry_after, reason) = match &result {
            Ok(r) if is_retryable(r.status()) => (retry_after(r), r.status().to_string()),
            Err(e) if !e.is_builder() => (None, e.to_string()),
            _ => return result,
        };
        if tries >= MAX_TRIES {
            return result;
        }
        warn!("Try {tries}/{MAX_TRIES} to fetch {what} from crates.io failed: {reason}");
        tokio::time::sleep(retry_after.unwrap_or(delay).min(MAX_RETRY_DELAY)).await;
        delay *= 2;
        tries += 1;
    }
}

fn is_retryable(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

/// The wait requested with a `Retry-After` header in seconds.
fn retry_after(response: &Response) -> Option<Duration> {
    response
        .headers()
        .get(reqwest::header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse()
        .ok()
        .map(Duration::from_secs)
}

#[derive(Default)]
struct CrateInfo {
    description: Option<String>,
    /// License per version, `UNKNOWN_LICENSE` if the version declares none.
    licenses: HashMap<String, String>,
}

fn parse_index(name: &OriginalName, data: &str) -> Option<Vec<IndexMetadata>> {
    match data
        .lines()
        .map(serde_json::from_str::<IndexMetadata>)
        .collect()
    {
        Ok(metadata) => Some(metadata),
        Err(e) => {
            error!("Could not parse prefetch data from crates.io for {name}: {e}");
            None
        }
    }
}

/// Path of a crate in the sparse index.
pub fn crate_sub_path(name: &OriginalName) -> String {
    // The index of crates.io uses the lowercase name with its separators
    let name = name.to_lowercase();
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
mod tests;
