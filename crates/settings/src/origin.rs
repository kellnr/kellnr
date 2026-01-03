use serde::{Deserialize, Serialize};

use crate::protocol::Protocol;

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone)]
#[serde(default)]
pub struct Origin {
    pub hostname: String,
    pub port: u16,
    pub protocol: Protocol,
    pub path: String,
}

impl Default for Origin {
    fn default() -> Self {
        // For local development the origin defaults to 127.0.0.1:8000.
        //
        // For integration tests (e.g. Docker Desktop on macOS) the host port is often
        // dynamically mapped. When Kellnr generates crates.io download URLs, it uses
        // `origin.hostname` + `origin.port`, so we must be able to override the port
        // at runtime.
        //
        // We support this via env var `KELLNR_ORIGIN__PORT` which mirrors how other
        // settings are configured.
        let port = std::env::var("KELLNR_ORIGIN__PORT")
            .ok()
            .and_then(|v| v.parse::<u16>().ok())
            .unwrap_or(8000);

        Self {
            hostname: "127.0.0.1".to_string(),
            port,
            protocol: Protocol::Http,
            path: String::new(),
        }
    }
}
