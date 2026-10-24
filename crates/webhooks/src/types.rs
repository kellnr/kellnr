use kellnr_common::webhook::{Webhook, WebhookEvent};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use utoipa::ToSchema;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RegisterWebhookRequest {
    // `type` alias included for webhook standards compatibility
    #[serde(alias = "type")]
    pub event: WebhookEvent,
    pub callback_url: String,
    pub name: Option<String>,
    /// Optional secret to sign deliveries according to the Standard Webhooks
    /// specification. Base64 encoded, optionally prefixed with `whsec_`, at
    /// least 24 bytes. The secret is never returned by the API.
    #[serde(default)]
    #[schema(write_only, example = "whsec_MfKQ9r8GKYqrTwjUPD8ILPZIo2LaLaSw")]
    pub secret: Option<String>,
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
    /// Whether deliveries are signed. The secret itself is never returned.
    #[serde(default)]
    pub has_secret: bool,
}

impl From<Webhook> for GetWebhookResponse {
    fn from(w: Webhook) -> Self {
        Self {
            id: w.id.unwrap_or_default(),
            event: w.event,
            callback_url: w.callback_url,
            name: w.name,
            has_secret: w.secret.is_some(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct GetAllWebhooksResponse(pub Vec<GetWebhookResponse>);

#[derive(Error, Debug)]
pub enum WebhookError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] kellnr_db::error::DbError),
}
