use common::webhook::{Webhook, WebhookAction};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisterWebhookRequest {
    pub action: WebhookAction,
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
    pub action: WebhookAction,
    pub callback_url: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetAllWebhooksResponse(pub Vec<Webhook>);

#[derive(Error, Debug)]
pub enum WebhookError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] db::error::DbError),
}
