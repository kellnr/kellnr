use kellnr_entity::auth_token;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct AuthToken {
    pub id: i32,
    pub name: String,
    #[serde(skip_serializing)]
    token: String,
}

impl AuthToken {
    pub fn new(id: i32, name: String, token: String) -> Self {
        Self { id, name, token }
    }
}

impl From<auth_token::Model> for AuthToken {
    fn from(m: auth_token::Model) -> Self {
        Self {
            id: m.id as i32,
            name: m.name,
            token: m.token,
        }
    }
}
