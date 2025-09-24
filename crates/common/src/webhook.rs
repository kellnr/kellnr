use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Webhook {
    pub id: Option<String>,
    pub action: WebhookAction,
    pub callback_url: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebhookAction {
    #[serde(rename = "crate_add")]
    CrateAdd,
    #[serde(rename = "crate_update")]
    CrateUpdate,
    #[serde(rename = "crate_yank")]
    CrateYank,
    #[serde(rename = "crate_unyank")]
    CrateUnyank,
}
impl From<WebhookAction> for &str {
    fn from(value: WebhookAction) -> Self {
        match value {
            WebhookAction::CrateAdd => "crate_add",
            WebhookAction::CrateUpdate => "crate_update",
            WebhookAction::CrateYank => "crate_yank",
            WebhookAction::CrateUnyank => "crate_unyank",
        }
    }
}
impl TryFrom<&str> for WebhookAction {
    type Error = String;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "crate_add" => Ok(Self::CrateAdd),
            "crate_update" => Ok(Self::CrateUpdate),
            "crate_yank" => Ok(Self::CrateYank),
            "crate_unyank" => Ok(Self::CrateUnyank),
            a => Err(format!("'{a}' is not a valid webhook action")),
        }
    }
}
