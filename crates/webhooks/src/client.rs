use std::time::Duration;

use chrono::Utc;
use kellnr_settings::Webhooks;
use reqwest::header::CONTENT_TYPE;
use serde::Serialize;
use thiserror::Error;

use crate::signature::{decode_secret, sign};
use crate::target::{GuardedResolver, is_blocked_url, validate_callback_url};

pub(crate) const MAX_LOGGED_BODY_BYTES: usize = 512;

#[derive(Error, Debug)]
pub(crate) enum DeliveryError {
    #[error("{0}")]
    InvalidUrl(String),
    #[error("Webhook target address is not permitted")]
    BlockedTarget,
    #[error("{0}")]
    InvalidSecret(String),
    #[error("Cannot serialize webhook payload: {0}")]
    Payload(#[from] serde_json::Error),
    #[error("Webhook request failed: {0}")]
    Request(#[from] reqwest::Error),
}

/// HTTP client used for delivering webhook callbacks.
///
/// Explicit connect and request timeouts prevent a slow or unresponsive
/// callback host from stalling the delivery worker (or a request handler)
/// indefinitely. Redirects are never followed, so a callback host cannot
/// bounce deliveries to a different target. Link-local and cloud metadata
/// addresses (and optionally all non-public addresses) are refused, both for
/// IP literals and after DNS resolution. With `ignore_env_proxy`, deliveries
/// do not use a proxy from the environment (`HTTP_PROXY`, `HTTPS_PROXY`), as
/// only the proxy's address can be checked then.
#[derive(Clone)]
pub(crate) struct WebhookClient {
    http: reqwest::Client,
    block_private_targets: bool,
}

impl WebhookClient {
    pub(crate) fn new(settings: &Webhooks) -> reqwest::Result<Self> {
        let block_private_targets = settings.block_private_targets;
        let mut builder = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .dns_resolver(GuardedResolver {
                block_private_targets,
            });
        if settings.ignore_env_proxy {
            builder = builder.no_proxy();
        }
        let http = builder.build()?;
        Ok(Self {
            http,
            block_private_targets,
        })
    }

    /// Sends `delivery` to `callback_url` with the Standard Webhooks headers
    /// `webhook-id` and `webhook-timestamp`. If the webhook has a secret, the
    /// delivery is signed with a `webhook-signature` header.
    pub(crate) async fn post<T: Serialize + ?Sized>(
        &self,
        callback_url: &str,
        delivery: &Delivery<'_, T>,
    ) -> Result<reqwest::Response, DeliveryError> {
        let url = validate_callback_url(callback_url).map_err(DeliveryError::InvalidUrl)?;
        if is_blocked_url(&url, self.block_private_targets) {
            return Err(DeliveryError::BlockedTarget);
        }

        let body = serde_json::to_vec(delivery.payload)?;
        let timestamp = Utc::now().timestamp();
        let mut request = self
            .http
            .post(url)
            .header(CONTENT_TYPE, "application/json")
            .header("webhook-id", delivery.id)
            .header("webhook-timestamp", timestamp);
        if let Some(secret) = delivery.secret {
            let key = decode_secret(secret).map_err(DeliveryError::InvalidSecret)?;
            request = request.header(
                "webhook-signature",
                sign(&key, delivery.id, timestamp, &body),
            );
        }
        Ok(request.body(body).send().await?)
    }
}

impl DeliveryError {
    /// Whether the delivery fails the same way on every attempt.
    pub(crate) fn is_permanent(&self) -> bool {
        matches!(
            self,
            Self::InvalidUrl(_) | Self::BlockedTarget | Self::InvalidSecret(_) | Self::Payload(_)
        )
    }
}

pub(crate) struct Delivery<'a, T: ?Sized> {
    /// Unique message id, identical for all delivery attempts of a message.
    pub(crate) id: &'a str,
    pub(crate) payload: &'a T,
    pub(crate) secret: Option<&'a str>,
}

/// Reads at most [`MAX_LOGGED_BODY_BYTES`] of a callback response body for
/// logging, without buffering an arbitrarily large response.
pub(crate) async fn truncated_body(mut response: reqwest::Response) -> String {
    let mut body = Vec::new();
    while body.len() < MAX_LOGGED_BODY_BYTES {
        match response.chunk().await {
            Ok(Some(chunk)) => body.extend_from_slice(&chunk),
            Ok(None) | Err(_) => break,
        }
    }
    body.truncate(MAX_LOGGED_BODY_BYTES);
    String::from_utf8_lossy(&body).into_owned()
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderMap;

    use super::*;
    use crate::tests::{get_redirect_listener, get_test_listener, get_test_listener_with_response};

    /// Environment variable with the target URL for the tests run in a child
    /// process by [`run_with_proxy_in_environment`].
    const TARGET_ENV: &str = "KELLNR_TEST_WEBHOOK_TARGET";

    #[tokio::test]
    async fn truncates_response_body() {
        let listener =
            get_test_listener_with_response(500, HeaderMap::new(), "x".repeat(10_000)).await;

        let response = client(false)
            .post(&listener.url(), &unsigned())
            .await
            .unwrap();

        assert_eq!(MAX_LOGGED_BODY_BYTES, truncated_body(response).await.len());
    }

    fn client(block_private_targets: bool) -> WebhookClient {
        WebhookClient::new(&Webhooks {
            block_private_targets,
            ..Webhooks::default()
        })
        .unwrap()
    }

    fn unsigned() -> Delivery<'static, str> {
        Delivery {
            id: "msg_1",
            payload: "payload",
            secret: None,
        }
    }

    fn header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
        headers.get(name).map(|value| value.to_str().unwrap())
    }

    #[tokio::test]
    async fn sends_signed_delivery_if_secret_is_set() {
        let mut listener = get_test_listener(200).await;
        let secret = "whsec_MfKQ9r8GKYqrTwjUPD8ILPZIo2LaLaSw";
        let payload = serde_json::json!({"type": "crate_add"});

        client(false)
            .post(
                &listener.url(),
                &Delivery {
                    id: "msg_42",
                    payload: &payload,
                    secret: Some(secret),
                },
            )
            .await
            .unwrap();
        let request = listener.rx.recv().await.unwrap();

        let timestamp: i64 = header(&request.headers, "webhook-timestamp")
            .unwrap()
            .parse()
            .unwrap();
        let expected = sign(
            &decode_secret(secret).unwrap(),
            "msg_42",
            timestamp,
            &request.body,
        );

        assert_eq!(Some("msg_42"), header(&request.headers, "webhook-id"));
        assert_eq!(
            Some(expected.as_str()),
            header(&request.headers, "webhook-signature")
        );
        assert_eq!(
            Some("application/json"),
            header(&request.headers, "content-type")
        );
        assert_eq!(
            payload,
            serde_json::from_slice::<serde_json::Value>(&request.body).unwrap()
        );
    }

    #[tokio::test]
    async fn sends_unsigned_delivery_without_secret() {
        let mut listener = get_test_listener(200).await;

        client(false)
            .post(&listener.url(), &unsigned())
            .await
            .unwrap();
        let request = listener.rx.recv().await.unwrap();

        assert_eq!(Some("msg_1"), header(&request.headers, "webhook-id"));
        assert!(header(&request.headers, "webhook-timestamp").is_some());
        assert!(header(&request.headers, "webhook-signature").is_none());
    }

    #[tokio::test]
    async fn does_not_follow_redirects() {
        let mut target = get_test_listener(200).await;
        let redirect = get_redirect_listener(&target.url()).await;

        let response = client(false)
            .post(&redirect.url(), &unsigned())
            .await
            .unwrap();

        assert_eq!(302, response.status().as_u16());
        assert!(target.rx.try_recv().is_err());
    }

    /// Runs `test` in a child process with `HTTP_PROXY` pointing at
    /// `proxy_url` and [`TARGET_ENV`] at `target_url`, as the environment of
    /// this process cannot be changed safely, and returns whether the child
    /// test passed.
    async fn run_with_proxy_in_environment(test: &str, proxy_url: &str, target_url: &str) -> bool {
        let mut child = std::process::Command::new(std::env::current_exe().unwrap());
        child
            .args(["--exact", test, "--ignored"])
            .env("HTTP_PROXY", proxy_url)
            .env("http_proxy", proxy_url)
            .env(TARGET_ENV, target_url)
            .env_remove("NO_PROXY")
            .env_remove("no_proxy");
        tokio::task::spawn_blocking(move || child.status().unwrap().success())
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn uses_proxy_from_environment_by_default() {
        let mut proxy = get_test_listener(200).await;
        let mut target = get_test_listener(200).await;

        let passed = run_with_proxy_in_environment(
            "client::tests::delivers_with_default_settings",
            &proxy.url(),
            &target.url(),
        )
        .await;

        assert!(passed);
        assert!(proxy.rx.try_recv().is_ok());
        assert!(target.rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn ignores_proxy_from_environment_if_configured() {
        let mut proxy = get_test_listener(200).await;
        let mut target = get_test_listener(200).await;

        let passed = run_with_proxy_in_environment(
            "client::tests::delivers_with_ignore_env_proxy",
            &proxy.url(),
            &target.url(),
        )
        .await;

        assert!(passed);
        assert!(proxy.rx.try_recv().is_err());
        assert!(target.rx.try_recv().is_ok());
    }

    #[tokio::test]
    #[ignore = "run by uses_proxy_from_environment_by_default"]
    async fn delivers_with_default_settings() {
        let response = client(false)
            .post(&std::env::var(TARGET_ENV).unwrap(), &unsigned())
            .await
            .unwrap();
        assert_eq!(200, response.status().as_u16());
    }

    #[tokio::test]
    #[ignore = "run by ignores_proxy_from_environment_if_configured"]
    async fn delivers_with_ignore_env_proxy() {
        let client = WebhookClient::new(&Webhooks {
            ignore_env_proxy: true,
            ..Webhooks::default()
        })
        .unwrap();
        let response = client
            .post(&std::env::var(TARGET_ENV).unwrap(), &unsigned())
            .await
            .unwrap();
        assert_eq!(200, response.status().as_u16());
    }

    #[tokio::test]
    async fn refuses_link_local_ip_literals() {
        let result = client(false)
            .post("http://169.254.169.254/latest/meta-data", &unsigned())
            .await;
        assert!(matches!(result, Err(DeliveryError::BlockedTarget)));
    }

    #[tokio::test]
    async fn refuses_private_targets_if_configured() {
        let mut target = get_test_listener(200).await;

        let result = client(true).post(&target.url(), &unsigned()).await;
        assert!(matches!(result, Err(DeliveryError::BlockedTarget)));

        let result = client(true)
            .post(&format!("http://localhost:{}", target.port()), &unsigned())
            .await;
        assert!(matches!(result, Err(DeliveryError::Request(_))));

        assert!(target.rx.try_recv().is_err());
    }
}
