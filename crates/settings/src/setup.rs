use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone)]
#[serde(default)]
pub struct Setup {
    pub admin_pwd: String,
    pub admin_token: String,
}

impl Default for Setup {
    fn default() -> Self {
        Self {
            admin_pwd: "admin".to_string(),
            admin_token: "Zy9HhJ02RJmg0GCrgLfaCVfU6IwDfhXD".to_string(),
        }
    }
}
