use std::sync::Arc;
use std::time::Duration;

use reqwest::{Client, ClientBuilder, StatusCode, Url};
use tracing::error;

/// Default user-agent sent with requests to crates.io. This is the single source
/// of truth for the default user-agent across the whole codebase.
pub const DEFAULT_USER_AGENT: &str = "kellnr.io/kellnr";

/// Build a reqwest client with the given user-agent and timeouts.
///
/// If `user_agent` is not a valid HTTP header value, the [`DEFAULT_USER_AGENT`]
/// is used instead.
pub fn build_client(
    user_agent: &str,
    connect_timeout: Duration,
    request_timeout: Duration,
) -> Client {
    let user_agent = reqwest::header::HeaderValue::from_str(user_agent).unwrap_or_else(|_| {
        error!("Invalid user-agent {user_agent:?}, falling back to default");
        reqwest::header::HeaderValue::from_static(DEFAULT_USER_AGENT)
    });
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(reqwest::header::USER_AGENT, user_agent);
    ClientBuilder::new()
        .gzip(true)
        .connect_timeout(connect_timeout)
        .timeout(request_timeout)
        .default_headers(headers)
        .build()
        .unwrap()
}

/// Default client with the default user-agent and sensible timeouts (5s connect, 30s request).
pub static CLIENT: std::sync::LazyLock<Client> = std::sync::LazyLock::new(|| {
    build_client(
        DEFAULT_USER_AGENT,
        Duration::from_secs(5),
        Duration::from_secs(30),
    )
});

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
