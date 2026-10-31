//! HTTP client adapter for the `openidconnect` crate.
//!
//! `openidconnect`'s built-in reqwest integration targets reqwest 0.12, which trusts
//! the compiled-in Mozilla root bundle and ignores the system trust store.
//! This adapter lets it use the reqwest version of Kellnr instead, which also
//! trusts the system trust store, so OIDC providers behind a private CA are
//! trusted the same way as every other outgoing request
//! (see <https://github.com/kellnr/kellnr/issues/1376>).

use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use openidconnect::{AsyncHttpClient, HttpClientError, HttpRequest, HttpResponse, http};

/// Time to connect to the identity provider.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Time for a whole request to the identity provider, so that a hanging
/// provider cannot block logins forever.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// A reqwest client that implements `openidconnect`'s [`AsyncHttpClient`].
#[derive(Clone, Debug)]
pub(crate) struct OidcHttpClient(reqwest::Client);

impl OidcHttpClient {
    /// Build a client that does not follow redirects, as recommended for
    /// `OAuth2` to prevent SSRF via redirecting endpoints.
    pub(crate) fn new() -> reqwest::Result<Self> {
        Self::with_timeouts(CONNECT_TIMEOUT, REQUEST_TIMEOUT)
    }

    fn with_timeouts(connect: Duration, request: Duration) -> reqwest::Result<Self> {
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(connect)
            .timeout(request)
            .build()
            .map(Self)
    }
}

impl<'c> AsyncHttpClient<'c> for OidcHttpClient {
    type Error = HttpClientError<reqwest::Error>;
    type Future =
        Pin<Box<dyn Future<Output = Result<HttpResponse, Self::Error>> + Send + Sync + 'c>>;

    fn call(&'c self, request: HttpRequest) -> Self::Future {
        Box::pin(async move {
            let request = request.try_into().map_err(Box::new)?;
            let response = self.0.execute(request).await.map_err(Box::new)?;

            let mut builder = http::Response::builder()
                .status(response.status())
                .version(response.version());
            for (name, value) in response.headers() {
                builder = builder.header(name, value);
            }

            let body = response.bytes().await.map_err(Box::new)?;
            builder.body(body.to_vec()).map_err(HttpClientError::Http)
        })
    }
}

/// Format an error together with its full source chain.
///
/// Errors from `openidconnect` only say "Request failed" at the top level; the
/// actual cause (TLS trust, DNS, connection refused, ...) is in the sources.
pub(crate) fn error_chain(error: &dyn Error) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        message.push_str(": ");
        message.push_str(&cause.to_string());
        source = cause.source();
    }
    message
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, thiserror::Error)]
    #[error("outer")]
    struct Outer(#[source] Inner);

    #[derive(Debug, thiserror::Error)]
    #[error("inner")]
    struct Inner;

    /// A provider that accepts the connection but never answers must not
    /// block the login forever.
    #[tokio::test]
    async fn hanging_provider_times_out() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (_socket, _) = listener.accept().await.unwrap();
            std::future::pending::<()>().await;
        });
        let client =
            OidcHttpClient::with_timeouts(Duration::from_secs(1), Duration::from_millis(100))
                .unwrap();
        let request = http::Request::get(format!("http://{addr}/"))
            .body(Vec::new())
            .unwrap();

        let result = tokio::time::timeout(Duration::from_secs(10), client.call(request))
            .await
            .expect("request must time out on its own");

        assert!(result.is_err());
    }

    #[test]
    fn error_chain_includes_all_sources() {
        assert_eq!(error_chain(&Outer(Inner)), "outer: inner");
    }

    #[test]
    fn error_chain_without_source_is_display() {
        assert_eq!(error_chain(&Inner), "inner");
    }
}
