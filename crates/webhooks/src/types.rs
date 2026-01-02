use kellnr_common::webhook::{Webhook, WebhookEvent};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisterWebhookRequest {
    // `type` alias included for webhook standards compatibility
    #[serde(alias = "type")]
    pub event: WebhookEvent,
    pub callback_url: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisterWebhookResponse {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetWebhookResponse {
    pub id: String,
    pub event: WebhookEvent,
    pub callback_url: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetAllWebhooksResponse(pub Vec<Webhook>);

#[derive(Error, Debug)]
pub enum WebhookError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] kellnr_db::error::DbError),
}
