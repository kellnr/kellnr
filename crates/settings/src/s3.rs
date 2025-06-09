use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone)]
pub struct S3 {
    pub enabled: bool,
    pub access_key: String,
    pub secret_key: String,
    pub region: String,
    pub endpoint: String,
    pub allow_http: bool,
    pub crates_bucket: String,
    pub cratesio_bucket: String,
}

impl Default for S3 {
    fn default() -> Self {
        Self {
            enabled: false,
            access_key: "minioadmin".to_string(),
            secret_key: "minioadmin".to_string(),
            region: "us-east-1".to_string(),
            endpoint: "http://localhost:9000/".to_string(),
            allow_http: true,
            crates_bucket: "kellnr-crates".to_string(),
            cratesio_bucket: "kellnr-cratesio".to_string(),
        }
    }
}
