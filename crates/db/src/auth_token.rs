use kellnr_entity::auth_token;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct AuthToken {
    pub id: i32,
    pub name: String,
    #[serde(skip_serializing)]
    token: String,
    /// When the token was created, unknown for tokens from before Kellnr
    /// stored it
    pub created: Option<String>,
    /// When the token was last used, updated at most once per hour
    pub last_used: Option<String>,
}

impl AuthToken {
    pub fn new(id: i32, name: String, token: String) -> Self {
        Self {
            id,
            name,
            token,
            created: None,
            last_used: None,
        }
    }
}

impl From<auth_token::Model> for AuthToken {
    fn from(m: auth_token::Model) -> Self {
        Self {
            id: m.id as i32,
            name: m.name,
            token: m.token,
            created: m.created,
            last_used: m.last_used,
        }
    }
}
