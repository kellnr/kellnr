use clap_serde_derive::ClapSerde;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone, ClapSerde)]
#[serde(default)]
pub struct S3 {
    /// Use S3 storage instead of filesystem
    #[default(false)]
    #[arg(id = "s3-enabled", long = "s3-enabled")]
    pub enabled: bool,

    /// S3 access key
    #[default(None)]
    #[arg(id = "s3-access-key", long = "s3-access-key")]
    pub access_key: Option<String>,

    /// S3 secret key
    #[default(None)]
    #[arg(id = "s3-secret-key", long = "s3-secret-key")]
    pub secret_key: Option<String>,

    /// S3 region
    #[default(None)]
    #[arg(id = "s3-region", long = "s3-region")]
    pub region: Option<String>,

    /// S3 endpoint URL
    #[default(None)]
    #[arg(id = "s3-endpoint", long = "s3-endpoint")]
    pub endpoint: Option<String>,

    /// Allow HTTP (non-TLS) connections
    #[default(false)]
    #[arg(id = "s3-allow-http", long = "s3-allow-http")]
    pub allow_http: bool,

    /// Bucket for kellnr crates
    #[default(None)]
    #[arg(id = "s3-crates-bucket", long = "s3-crates-bucket")]
    pub crates_bucket: Option<String>,

    /// Bucket for cached crates.io crates
    #[default(None)]
    #[arg(id = "s3-cratesio-bucket", long = "s3-cratesio-bucket")]
    pub cratesio_bucket: Option<String>,
}
