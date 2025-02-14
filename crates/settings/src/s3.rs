use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone)]
pub struct S3 {
    pub enabled: bool,
    pub access_key: String,
    pub secret_key: String,
    pub bucket: String,
    pub region: String,
    pub endpoint: String,
    pub allow_http: bool,
}

impl Default for S3 {
    fn default() -> Self {
        Self {
            enabled: false,
            access_key: String::from("minio"),
            secret_key: String::from("minio123"),
            bucket: String::from("my-bucket"),
            region: String::from("us-east-1"),
            endpoint: String::from("http://localhost:9000/"),
            allow_http: true,
        }
    }
}
