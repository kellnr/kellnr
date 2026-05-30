use provcfg::{ClapArgs, Configurable};
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

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone, Configurable, ClapArgs)]
#[serde(default)]
#[configurable(clap_prefix = "origin")]
pub struct Origin {
    /// External hostname for URLs
    pub hostname: String,

    /// External port for URLs
    pub port: u16,

    /// Protocol (http or https), not exposed on the CLI; set via TOML/env
    #[arg(skip)]
    pub protocol: Protocol,

    /// URL path prefix
    pub path: String,
}

impl Default for Origin {
    fn default() -> Self {
        Self {
            hostname: default_hostname(),
            port: default_origin_port(),
            protocol: Protocol::Http,
            path: String::new(),
        }
    }
}
