use std::str::FromStr;

use clap_serde_derive::ClapSerde;
use clap::{Parser, ValueEnum};
use serde::{Deserialize, Deserializer, Serialize};
use url::Url;
use crate::deserialize_with::DeserializeWith;
use crate::s3::S3Config;
use serde::de::Error;


#[derive(Debug, Eq, PartialEq, Clone, ClapSerde)]
pub struct FileConfig {
    pub folder: String
}

impl FileConfig {
    pub fn new(s: impl AsRef<str>) -> Self {
        Self {
            folder: s.as_ref().to_string()
        }
    }
}

impl FromStr for FileConfig {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s
        .strip_prefix("folder=")
        .map(FileConfig::new)
        .ok_or(format!("Invalid value for file config: {}", s))
    }
}

impl<'de> Deserialize<'de> for FileConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de> {
        
        let s = String::deserialize(deserializer)?;

        s
        .parse()
        .map_err(D::Error::custom)
    }
}

impl std::fmt::Display for FileConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "folder={}", self.folder.as_str())
    }
}


#[derive(Debug, Eq, PartialEq, Clone)]
pub enum StorageBackend {
    File(FileConfig),
    S3(S3Config),
}

impl StorageBackend {
    pub fn file(config: FileConfig) -> Self {
        Self::File(config)
    }
    pub fn s3(config: S3Config) -> Self {
        Self::S3(config)
    }

    pub fn is_s3(&self) -> bool {
        matches!(self, Self::S3(_))
    }

    pub fn is_file(&self) -> bool {
        matches!(self, Self::File(_))
    }

    pub fn file_config(&self) -> Option<&FileConfig> {
        match self {
            Self::File(config) => Some(config),
            Self::S3(_) => None
        }
    }
    pub fn s3_config(&self) -> Option<&S3Config> {
        match self {
            Self::File(_) => None,
            Self::S3(config) => Some(config)
        }
    }
}

impl<'de> Deserialize<'de> for StorageBackend {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de> {
        
        String::deserialize(deserializer)?
        .parse()
        .map_err(D::Error::custom)
    }
}

impl Serialize for StorageBackend {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer 
    {
        serializer.serialize_str(&self.to_string())
    }
}


impl FromStr for StorageBackend {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {

        if let Some(suffix) = s.strip_prefix("kind=file,") {
            return Ok(
                suffix
                .parse::<FileConfig>()
                .map(|config| Self::File(config))
                .map_err(|err| format!("unable to parse file config: {err}"))?
            );
        }

        if let Some(suffix) = s.strip_prefix("kind=s3,") {
            return Ok(
                suffix
                .parse::<S3Config>()
                .map(|config| Self::S3(config))
                .map_err(|err| format!("unable to parse s3 config: {err}"))?
            );
        }
        Err(format!("Invalid configuration for storage backend. should start with either kind=s3, or kind=file: '{s}'"))
    }
}

impl std::fmt::Display for StorageBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::File(config) => {
                write!(f, "kind=file,{}", config)
            },
            Self::S3(config) => {
                write!(f, "kind=s3,{}", config)
            }
        }
    }
}


#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Clone, ClapSerde)]
pub struct Storage {
    #[default(StorageBackend::File(FileConfig::new("crates")))]
    #[arg(id = "storage-kellnr-crates", long = "storage-kellnr-crates")]
    pub kellnr_crates: StorageBackend,
    #[default(StorageBackend::File(FileConfig::new("crates-io")))]
    #[arg(id = "storage-crates-io", long = "storage-crates-io")]
    pub crates_io: StorageBackend,
    #[default(StorageBackend::File(FileConfig::new("toolchain")))]
    #[arg(id = "storage-toolchain", long = "storage-toolchain")]
    pub toolchain: StorageBackend,
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_serde() {
        let toml = r#"
            kellnr_crates = "kind=file,folder=foo"
            crates_io = "kind=s3,access_key=foo,region=bar,endpoint=foo.com,secret_key=,bucket=baz,allow_http=false"
        "#;
        let storage: Storage = toml::from_str(&toml).unwrap();

        let crates_io = storage.crates_io;
        assert!(crates_io.is_s3());
        let crates_io_config = crates_io.s3_config().unwrap();
        assert_eq!(crates_io_config.access_key, Some("foo".to_string()));
        assert_eq!(crates_io_config.secret_key, None);
        assert_eq!(crates_io_config.region, Some("bar".to_string()));
        assert_eq!(crates_io_config.endpoint, Some("foo.com".to_string()));
        assert_eq!(crates_io_config.bucket, "baz".to_string());
        assert!(!crates_io_config.allow_http);

        let kellnr_crates = storage.kellnr_crates;
        assert!(kellnr_crates.is_file());
        let kellnr_crates_config = kellnr_crates.file_config().unwrap();
        assert_eq!(kellnr_crates_config.folder, "foo".to_string());

        assert!(storage.toolchain.is_file());
        let toolchain_config = storage.toolchain.file_config().unwrap();
        assert_eq!(toolchain_config.folder, "toolchain".to_string());
    }

    #[test]
    fn test_storage_default() {
        let storage: Storage = Default::default();
        assert_eq!(storage.kellnr_crates.file_config().unwrap().folder, "crates".to_string());
        assert_eq!(storage.crates_io.file_config().unwrap().folder, "crates-io".to_string());
        assert_eq!(storage.toolchain.file_config().unwrap().folder, "toolchain".to_string());

    }
}