use clap_serde_derive::ClapSerde;
use serde::{Deserialize, Serialize};

use crate::protocol::Protocol;

fn default_hostname() -> String {
    "127.0.0.1".to_string()
}

fn default_origin_port() -> u16 {
    std::env::var("KELLNR_ORIGIN__PORT")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(8000)
}

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone, ClapSerde)]
#[serde(default)]
pub struct Origin {
    /// External hostname for URLs
    #[default(default_hostname())]
    #[arg(id = "origin-hostname", long = "origin-hostname")]
    pub hostname: String,

    /// External port for URLs
    #[default(default_origin_port())]
    #[arg(id = "origin-port", long = "origin-port")]
    pub port: u16,

    /// Protocol (http or https)
    #[default(Protocol::Http)]
    #[arg(skip)]
    pub protocol: Protocol,

    /// URL path prefix
    #[default(String::new())]
    #[arg(id = "origin-path", long = "origin-path")]
    pub path: String,
}
