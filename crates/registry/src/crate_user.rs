use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CrateUser {
    pub id: i32,
    pub login: String,
    pub name: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CrateUserList {
    pub users: Vec<CrateUser>,
}

impl From<Vec<CrateUser>> for CrateUserList {
    fn from(users: Vec<CrateUser>) -> Self {
        Self { users }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CrateUserRequest {
    pub users: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CrateUserResponse {
    pub ok: bool,
    pub msg: String,
}

impl From<&str> for CrateUserResponse {
    fn from(msg: &str) -> Self {
        Self {
            ok: true,
            msg: msg.to_string(),
        }
    }
}
