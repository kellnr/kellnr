use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Webhook {
    pub id: Option<String>,
    // `type` alias included for webhook standards compatibility
    #[serde(alias = "type")]
    pub event: WebhookEvent,
    pub callback_url: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebhookQueue {
    pub id: String,
    pub callback_url: String,
    pub payload: serde_json::Value,
    pub last_attempt: Option<DateTime<Utc>>,
    pub next_attempt: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebhookEvent {
    #[serde(rename = "crate_add")]
    CrateAdd,
    #[serde(rename = "crate_update")]
    CrateUpdate,
    #[serde(rename = "crate_yank")]
    CrateYank,
    #[serde(rename = "crate_unyank")]
    CrateUnyank,
}
impl From<WebhookEvent> for &str {
    fn from(value: WebhookEvent) -> Self {
        match value {
            WebhookEvent::CrateAdd => "crate_add",
            WebhookEvent::CrateUpdate => "crate_update",
            WebhookEvent::CrateYank => "crate_yank",
            WebhookEvent::CrateUnyank => "crate_unyank",
        }
    }
}
impl TryFrom<&str> for WebhookEvent {
    type Error = String;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "crate_add" => Ok(Self::CrateAdd),
            "crate_update" => Ok(Self::CrateUpdate),
            "crate_yank" => Ok(Self::CrateYank),
            "crate_unyank" => Ok(Self::CrateUnyank),
            a => Err(format!("'{a}' is not a valid webhook event")),
        }
    }
}
