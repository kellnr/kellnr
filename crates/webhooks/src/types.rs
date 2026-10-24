use std::time::Duration;

use kellnr_common::webhook::{Webhook, WebhookEvent};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use utoipa::ToSchema;

/// Build the HTTP client used for delivering webhook callbacks.
///
/// Explicit connect and request timeouts prevent a slow or unresponsive
/// callback host from stalling the delivery worker (or a request handler)
/// indefinitely. Redirects are never followed, so a callback host cannot
/// bounce deliveries to a different target.
pub fn build_client() -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RegisterWebhookRequest {
    // `type` alias included for webhook standards compatibility
    #[serde(alias = "type")]
    pub event: WebhookEvent,
    pub callback_url: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RegisterWebhookResponse {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct GetWebhookResponse {
    pub id: String,
    pub event: WebhookEvent,
    pub callback_url: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct GetAllWebhooksResponse(pub Vec<Webhook>);

#[derive(Error, Debug)]
pub enum WebhookError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] kellnr_db::error::DbError),
}

#[cfg(test)]
mod types_tests {
    use super::*;
    use crate::tests::{get_redirect_listener, get_test_listener};

    #[tokio::test]
    async fn build_client_does_not_follow_redirects() {
        let mut target = get_test_listener(9983, 200).await;
        let _redirect = get_redirect_listener(9984, "http://127.0.0.1:9983/").await;

        let response = build_client()
            .unwrap()
            .post("http://127.0.0.1:9984")
            .send()
            .await
            .unwrap();

        assert_eq!(302, response.status().as_u16());
        assert!(target.rx.try_recv().is_err());
    }
}
