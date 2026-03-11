use std::str::FromStr;

use clap_serde_derive::ClapSerde;
use serde::{Deserialize, Deserializer, Serialize, de::Error};

#[derive(Debug, Eq, PartialEq, Clone, ClapSerde)]
#[serde(default)]
pub struct S3Config {

    #[default(None)]
    #[arg(id = "s3-config-access-key", long = "s3-config-access-key")]
    pub access_key: Option<String>,

    #[default(None)]
    #[arg(id = "s3-config-secret-key", long = "s3-config-secret-key")]
    pub secret_key: Option<String>,

    #[default(None)]
    #[arg(id = "s3-config-region", long = "s3-config-region")]
    pub region: Option<String>,

    #[default(None)]
    #[arg(id = "s3-config-endpoint", long = "s3-config-endpoint")]
    pub endpoint: Option<String>,

    #[default(true)]
    #[arg(id = "s3-config-allow-http", long = "s3-config-allow-http")]
    pub allow_http: bool,

    #[default("kellnr".to_string())]
    #[arg(id = "s3-config-bucket", long = "s3-config-bucket")]
    pub bucket: String,
}

impl FromStr for S3Config {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut s3_config = S3Config::default();

        for keyval in s.split(',') {
            if let Some((key, val)) = keyval.split_once('=') {
                match key {
                    "access_key" => {
                        if !val.is_empty() {
                            s3_config.access_key = Some(val.to_string());
                        }
                    },
                    "secret_key" => {
                        if !val.is_empty() {
                            s3_config.secret_key = Some(val.to_string());
                        }
                    },
                    "region" => {
                        if !val.is_empty() {
                            s3_config.region = Some(val.to_string());
                        }
                    },
                    "endpoint" => {
                        if !val.is_empty() {
                            s3_config.endpoint = Some(val.to_string());
                        }
                    },
                    "allow_http" => {
                        if !val.is_empty() {
                            match val {
                                "1" | "true" => {
                                    s3_config.allow_http = true;
                                },
                                "0" | "false" => {
                                    s3_config.allow_http = false;
                                },
                                _ => {
                                    return Err(format!("Invalid value for 'allow_http' in S3 configuration: {val}"))
                                }
                            }
                        }
                    },
                    "bucket" => {
                        if !val.is_empty() {
                            s3_config.bucket = val.to_string();
                        }
                    },
                    _ => {
                        return Err(format!("Invalid key for S3 configuration: {key}"))
                    }
                }
            }
        }

        Ok(s3_config)
    }
}

impl std::fmt::Display for S3Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "access_key={},", self.access_key.as_deref().unwrap_or_default())?;
        write!(f, "secret_key={},", self.secret_key.as_deref().unwrap_or_default())?;
        write!(f, "region={},", self.region.as_deref().unwrap_or_default())?;
        write!(f, "endpoint={},", self.endpoint.as_deref().unwrap_or_default())?;
        write!(f, "bucket={},", self.bucket.as_str())?;
        write!(f, "allow_http={}", self.allow_http)
    }
}

impl Serialize for S3Config {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer {
        serializer.serialize_str(&self.to_string())
    }
}


impl<'de> Deserialize<'de> for S3Config {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de> {
        
        let s = String::deserialize(deserializer)?;

        s
        .parse()
        .map_err(D::Error::custom)
    }
}