use kellnr_entity::user;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::UserId;

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct User {
    #[serde(skip_serializing)]
    pub id: UserId,
    pub name: String,
    #[serde(skip_serializing)]
    pub pwd: String,
    #[serde(skip_serializing)]
    pub salt: String,
    pub is_admin: bool,
    pub is_read_only: bool,
    pub created: String,
}

impl From<user::Model> for User {
    fn from(u: user::Model) -> Self {
        Self {
            id: UserId::new(u.id),
            name: u.name,
            pwd: u.pwd,
            salt: u.salt,
            is_admin: u.is_admin,
            is_read_only: u.is_read_only,
            created: u.created,
        }
    }
}
