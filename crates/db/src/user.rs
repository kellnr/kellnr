use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    #[serde(skip_serializing)]
    pub id: i32,
    pub name: String,
    #[serde(skip_serializing)]
    pub pwd: String,
    #[serde(skip_serializing)]
    pub salt: String,
    pub is_admin: bool,
}
