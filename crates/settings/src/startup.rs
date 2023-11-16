use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone)]
pub struct Startup {
    pub admin_pwd: String,
    pub admin_token: String,
}

impl Default for Startup {
    fn default() -> Self {
        Self {
            admin_pwd: String::from("admin"),
            admin_token: String::from("Zy9HhJ02RJmg0GCrgLfaCVfU6IwDfhXD"),
        }
    }
}
