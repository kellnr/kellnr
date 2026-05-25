use provcfg::{ClapArgs, Configurable};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone, Configurable, ClapArgs)]
#[serde(default)]
#[configurable(clap_prefix = "s3")]
pub struct S3 {
    pub enabled: bool,

    pub access_key: Option<String>,

    #[configurable(secret)]
    pub secret_key: Option<String>,

    pub region: Option<String>,

    pub endpoint: Option<String>,

    pub allow_http: bool,

    pub crates_bucket: String,

    pub cratesio_bucket: String,

    pub toolchain_bucket: String,

    /// S3 connect timeout in seconds
    #[arg(long = "s3-connect-timeout")]
    pub connect_timeout_seconds: u64,

    /// S3 request timeout in seconds
    #[arg(long = "s3-request-timeout")]
    pub request_timeout_seconds: u64,
}

impl Default for S3 {
    fn default() -> Self {
        Self {
            enabled: false,
            access_key: None,
            secret_key: None,
            region: None,
            endpoint: None,
            allow_http: true,
            crates_bucket: "kellnr-crates".to_string(),
            cratesio_bucket: "kellnr-cratesio".to_string(),
            toolchain_bucket: "kellnr-toolchains".to_string(),
            connect_timeout_seconds: 5,
            request_timeout_seconds: 30,
        }
    }
}
