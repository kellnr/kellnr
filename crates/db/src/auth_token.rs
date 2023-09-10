use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
