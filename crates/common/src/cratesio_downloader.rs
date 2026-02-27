use std::sync::Arc;
use std::time::Duration;

use reqwest::{Client, ClientBuilder, StatusCode, Url};
use tracing::error;

/// Build a reqwest client with the given timeouts.
pub fn build_client(connect_timeout: Duration, request_timeout: Duration) -> Client {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::USER_AGENT,
        reqwest::header::HeaderValue::from_static("kellnr.io/kellnr"),
    );
    ClientBuilder::new()
        .gzip(true)
        .connect_timeout(connect_timeout)
        .timeout(request_timeout)
        .default_headers(headers)
        .build()
        .unwrap()
}

/// Default client with sensible timeouts (5s connect, 30s request).
pub static CLIENT: std::sync::LazyLock<Client> =
    std::sync::LazyLock::new(|| build_client(Duration::from_secs(5), Duration::from_secs(30)));

#[derive(Debug, thiserror::Error)]
pub enum DownloadCrateError {
    #[error("Crate not found")]
    NotFound,

    #[error("Received unexpected status code: {0}")]
    NotOk(StatusCode),

    #[error("Unexpected error while downloading crate: {0}")]
    Unexpected(reqwest::Error),

    #[error("Failed to parse response: {0}")]
    CannotParseResponse(reqwest::Error),

    #[error("Failed to parse URL: {0}")]
    CannotParseUrl(#[from] url::ParseError),
}

impl From<DownloadCrateError> for StatusCode {
    fn from(error: DownloadCrateError) -> Self {
        match error {
            DownloadCrateError::NotFound => StatusCode::NOT_FOUND,
            DownloadCrateError::NotOk(status) => status,
            DownloadCrateError::Unexpected(_)
            | DownloadCrateError::CannotParseResponse(_)
            | DownloadCrateError::CannotParseUrl(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

pub async fn download_crate(
    client: &Client,
    name: &str,
    version: &str,
    url: &Url,
) -> Result<Arc<[u8]>, DownloadCrateError> {
    let path = format!("{name}/{version}/download");
    let target = url.join(&path)?;

    let res = match client.get(target).send().await {
        Ok(resp) if resp.status() == 404 => Err(DownloadCrateError::NotFound),
        Ok(resp) if resp.status() == 403 => Err(DownloadCrateError::NotFound), // Map 403 to 404 as
        // crates.io returns 403 for non-existent crates
        Ok(resp) if resp.status() != 200 => Err(DownloadCrateError::NotOk(resp.status())),
        Ok(resp) => Ok(resp),
        Err(e) => {
            error!("Encountered error... {e}");
            Err(DownloadCrateError::Unexpected(e))
        }
    }?;

    let crate_data = res
        .bytes()
        .await
        .map_err(DownloadCrateError::CannotParseResponse)?;
    let crate_data: Arc<[u8]> = Arc::from(crate_data.iter().as_slice());

    Ok(crate_data)
}
