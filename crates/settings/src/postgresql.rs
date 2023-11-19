use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone)]
pub struct Postgresql {
    pub enabled: bool,
    pub address: String,
    pub port: u16,
    pub db: String,
    pub user: String,
    #[serde(skip_serializing, default)]
    pub pwd: String,
}

impl Default for Postgresql {
    fn default() -> Self {
        Self {
            enabled: false,
            address: String::from("localhost"),
            port: 5432,
            db: String::from("kellnr"),
            user: String::from(""),
            pwd: String::from(""),
        }
    }
}
