use kellnr_entity::auth_token;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::AuthTokenId;

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct AuthToken {
    pub id: AuthTokenId,
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
    pub fn new(id: AuthTokenId, name: String, token: String) -> Self {
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
            id: AuthTokenId::new(m.id),
            name: m.name,
            token: m.token,
            created: m.created,
            last_used: m.last_used,
        }
    }
}
