use std::time::Duration;

use kellnr_common::webhook::{Webhook, WebhookEvent};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use utoipa::ToSchema;

/// Build the HTTP client used for delivering webhook callbacks.
///
/// Explicit connect and request timeouts prevent a slow or unresponsive
/// callback host from stalling the delivery worker (or a request handler)
/// indefinitely. Falls back to a default client if the builder fails.
pub fn build_client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap_or_default()
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
