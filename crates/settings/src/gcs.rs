use provcfg::{ClapArgs, Configurable};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone, Configurable, ClapArgs)]
#[serde(default)]
#[configurable(clap_prefix = "gcs")]
pub struct Gcs {
    pub enabled: bool,

    /// Base URL override for self-hosted/emulator endpoints (e.g. fake-gcs-server in tests).
    /// When set, request signing is skipped since such endpoints are unauthenticated.
    /// When unset, credentials come from the environment (Application Default Credentials).
    pub endpoint: Option<String>,

    pub crates_bucket: String,

    pub cratesio_bucket: String,

    pub toolchain_bucket: String,

    /// GCS connect timeout in seconds
    #[arg(long = "gcs-connect-timeout")]
    pub connect_timeout_seconds: u64,

    /// GCS request timeout in seconds
    #[arg(long = "gcs-request-timeout")]
    pub request_timeout_seconds: u64,
}

impl Default for Gcs {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: None,
            crates_bucket: "kellnr-crates".to_string(),
            cratesio_bucket: "kellnr-cratesio".to_string(),
            toolchain_bucket: "kellnr-toolchains".to_string(),
            connect_timeout_seconds: 5,
            request_timeout_seconds: 30,
        }
    }
}
