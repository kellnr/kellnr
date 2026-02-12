use clap_serde_derive::ClapSerde;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone, ClapSerde)]
#[serde(default)]
pub struct S3 {
    #[default(false)]
    #[arg(id = "s3-enabled", long = "s3-enabled")]
    pub enabled: bool,

    #[default("access-key".to_string())]
    #[arg(id = "s3-access-key", long = "s3-access-key")]
    pub access_key: String,

    #[default("secret-key".to_string())]
    #[arg(id = "s3-secret-key", long = "s3-secret-key")]
    pub secret_key: String,

    #[default("us-east-1".to_string())]
    #[arg(id = "s3-region", long = "s3-region")]
    pub region: String,

    #[default("http://localhost:9000".to_string())]
    #[arg(id = "s3-endpoint", long = "s3-endpoint")]
    pub endpoint: String,

    #[default(true)]
    #[arg(id = "s3-allow-http", long = "s3-allow-http")]
    pub allow_http: bool,

    #[default("kellnr-crates".to_string())]
    #[arg(id = "s3-crates-bucket", long = "s3-crates-bucket")]
    pub crates_bucket: String,

    #[default("kellnr-cratesio".to_string())]
    #[arg(id = "s3-cratesio-bucket", long = "s3-cratesio-bucket")]
    pub cratesio_bucket: String,

    #[default("kellnr-toolchains".to_string())]
    #[arg(id = "s3-toolchain-bucket", long = "s3-toolchain-bucket")]
    pub toolchain_bucket: String,
}
