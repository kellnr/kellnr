use crate::protocol::Protocol;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone)]
pub struct Origin {
    pub protocol: Protocol,
    pub hostname: String,
    pub port: u16,
    pub path_prefix: Option<Box<str>>,
}

impl Default for Origin {
    fn default() -> Self {
        Self {
            protocol: Protocol::Http,
            hostname: String::from("127.0.0.1"),
            port: 8000,
            path_prefix: None,
        }
    }
}
