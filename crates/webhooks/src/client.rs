use std::time::Duration;

use kellnr_settings::Webhooks;
use serde::Serialize;
use thiserror::Error;

use crate::target::{GuardedResolver, is_blocked_url, validate_callback_url};

#[derive(Error, Debug)]
pub enum DeliveryError {
    #[error("{0}")]
    InvalidUrl(String),
    #[error("Webhook target address is not permitted")]
    BlockedTarget,
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
/// IP literals and after DNS resolution.
#[derive(Clone)]
pub struct WebhookClient {
    http: reqwest::Client,
    block_private_targets: bool,
}

impl WebhookClient {
    pub fn new(settings: &Webhooks) -> reqwest::Result<Self> {
        let block_private_targets = settings.block_private_targets;
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .dns_resolver(GuardedResolver {
                block_private_targets,
            })
            .build()?;
        Ok(Self {
            http,
            block_private_targets,
        })
    }

    pub(crate) async fn post<T: Serialize + ?Sized>(
        &self,
        callback_url: &str,
        payload: &T,
    ) -> Result<reqwest::Response, DeliveryError> {
        let url = validate_callback_url(callback_url).map_err(DeliveryError::InvalidUrl)?;
        if is_blocked_url(&url, self.block_private_targets) {
            return Err(DeliveryError::BlockedTarget);
        }
        Ok(self.http.post(url).json(payload).send().await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{get_redirect_listener, get_test_listener};

    fn client(block_private_targets: bool) -> WebhookClient {
        WebhookClient::new(&Webhooks {
            block_private_targets,
        })
        .unwrap()
    }

    #[tokio::test]
    async fn does_not_follow_redirects() {
        let mut target = get_test_listener(9983, 200).await;
        let _redirect = get_redirect_listener(9984, "http://127.0.0.1:9983/").await;

        let response = client(false)
            .post("http://127.0.0.1:9984", "payload")
            .await
            .unwrap();

        assert_eq!(302, response.status().as_u16());
        assert!(target.rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn refuses_link_local_ip_literals() {
        let result = client(false)
            .post("http://169.254.169.254/latest/meta-data", "payload")
            .await;
        assert!(matches!(result, Err(DeliveryError::BlockedTarget)));
    }

    #[tokio::test]
    async fn refuses_private_targets_if_configured() {
        let mut target = get_test_listener(9985, 200).await;

        let result = client(true).post("http://127.0.0.1:9985", "payload").await;
        assert!(matches!(result, Err(DeliveryError::BlockedTarget)));

        let result = client(true).post("http://localhost:9985", "payload").await;
        assert!(matches!(result, Err(DeliveryError::Request(_))));

        assert!(target.rx.try_recv().is_err());
    }
}
