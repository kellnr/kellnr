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
            access_key: String::from("minioadmin"),
            secret_key: String::from("minioadmin"),
            region: String::from("us-east-1"),
            endpoint: String::from("http://localhost:9000/"),
            allow_http: true,
            crates_bucket: String::from("kellnr-crates"),
            cratesio_bucket: String::from("kellnr-cratesio"),
        }
    }
}
