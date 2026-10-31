use kellnr_common::account_name::UserName;
use kellnr_entity::user;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::UserId;

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct User {
    #[serde(skip_serializing)]
    pub id: UserId,
    pub name: UserName,
    #[serde(skip_serializing)]
    pub pwd: String,
    #[serde(skip_serializing)]
    pub salt: String,
    pub is_admin: bool,
    pub is_read_only: bool,
    pub created: String,
}

/// The rights of a user, stored in the `is_admin` and `is_read_only`
/// columns. An admin can be read-only as well.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserFlags {
    pub admin: bool,
    pub read_only: bool,
}

impl UserFlags {
    pub const fn normal() -> Self {
        Self {
            admin: false,
            read_only: false,
        }
    }

    pub const fn admin() -> Self {
        Self {
            admin: true,
            read_only: false,
        }
    }

    pub const fn read_only() -> Self {
        Self {
            admin: false,
            read_only: true,
        }
    }
}

impl From<user::Model> for User {
    fn from(u: user::Model) -> Self {
        Self {
            id: UserId::new(u.id),
            name: UserName::from_unchecked(u.name),
            pwd: u.pwd,
            salt: u.salt,
            is_admin: u.is_admin,
            is_read_only: u.is_read_only,
            created: u.created,
        }
    }
}
