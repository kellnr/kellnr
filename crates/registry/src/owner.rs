use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Owner {
    pub id: i32,
    pub login: String,
    pub name: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerList {
    pub users: Vec<Owner>,
}

impl From<Vec<Owner>> for OwnerList {
    fn from(users: Vec<Owner>) -> Self {
        Self { users }
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerRequest {
    pub users: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerResponse {
    pub ok: bool,
    pub msg: String,
}

impl From<&str> for OwnerResponse {
    fn from(msg: &str) -> Self {
        Self {
            ok: true,
            msg: msg.to_string(),
        }
    }
}
