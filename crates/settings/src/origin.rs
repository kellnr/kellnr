use crate::protocol::Protocol;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone)]
pub struct Origin {
    pub hostname: String,
    pub port: u16,
    pub protocol: Protocol,
}

impl Default for Origin {
    fn default() -> Self {
        Self {
            hostname: String::from("127.0.0.1"),
            port: 8000,
            protocol: Protocol::Http,
        }
    }
}
