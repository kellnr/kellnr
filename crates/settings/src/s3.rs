use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize, Eq, PartialEq, Clone)]
pub struct S3 {
    pub enabled: bool,
    pub access_key: Option<String>,
    pub secret_key: Option<String>,
    pub region: Option<String>,
    pub endpoint: Option<String>,
    pub allow_http: bool,
    pub crates_bucket: Option<String>,
    pub cratesio_bucket: Option<String>,
}
