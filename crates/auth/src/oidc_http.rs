//! HTTP client adapter for the `openidconnect` crate.
//!
//! `openidconnect`'s built-in reqwest integration targets reqwest 0.12, which trusts
//! the compiled-in Mozilla root bundle and ignores the system trust store.
//! This adapter lets it use Kellnr's reqwest client instead, so OIDC providers
//! behind a private CA are trusted the same way as every other outgoing
//! request (see <https://github.com/kellnr/kellnr/issues/1376>).

use std::error::Error;
use std::future::Future;
use std::pin::Pin;

use openidconnect::{AsyncHttpClient, HttpClientError, HttpRequest, HttpResponse, http};

/// A reqwest client that implements `openidconnect`'s [`AsyncHttpClient`].
#[derive(Clone, Debug)]
pub struct OidcHttpClient(reqwest::Client);

impl OidcHttpClient {
    /// Build a client that does not follow redirects, as recommended for
    /// `OAuth2` to prevent SSRF via redirecting endpoints.
    pub fn new() -> reqwest::Result<Self> {
        reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
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
pub fn error_chain(error: &dyn Error) -> String {
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

    #[test]
    fn error_chain_includes_all_sources() {
        assert_eq!(error_chain(&Outer(Inner)), "outer: inner");
    }

    #[test]
    fn error_chain_without_source_is_display() {
        assert_eq!(error_chain(&Inner), "inner");
    }
}
