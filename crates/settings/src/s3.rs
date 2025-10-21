use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone)]
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

impl Default for S3 {
    fn default() -> Self {
        Self {
            enabled: false,
            access_key: Some("minioadmin".to_string()),
            secret_key: Some("minioadmin".to_string()),
            region: Some("us-east-1".to_string()),
            endpoint: Some("http://localhost:9000/".to_string()),
            allow_http: true,
            crates_bucket: Some("kellnr-crates".to_string()),
            cratesio_bucket: Some("kellnr-cratesio".to_string()),
        }
    }
}
