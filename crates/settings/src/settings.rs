use std::convert::TryFrom;
use std::env;
use std::path::{Path, PathBuf};

use config::{Config, ConfigError, Environment, File};
use serde::{Deserialize, Serialize};

use crate::compile_time_config;
use crate::docs::Docs;
use crate::local::Local;
use crate::log::Log;
use crate::origin::Origin;
use crate::postgresql::Postgresql;
use crate::proxy::Proxy;
use crate::registry::Registry;
use crate::s3::S3;
use crate::setup::Setup;

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Default, Clone)]
#[serde(default)]
pub struct Settings {
    pub setup: Setup,
    pub registry: Registry,
    pub docs: Docs,
    pub proxy: Proxy,
    pub log: Log,
    pub local: Local,
    pub origin: Origin,
    pub postgresql: Postgresql,
    pub s3: S3,
}

impl TryFrom<Option<&Path>> for Settings {
    type Error = ConfigError;

    fn try_from(config_path: Option<&Path>) -> Result<Self, Self::Error> {
        let env = env::var("RUN_MODE").unwrap_or_else(|_| "development".into());
        let builder = Config::builder();

        // Add configuration values from a config file if a config path is provided
        let builder = match config_path {
            Some(config_path) => {
                let default_file = Settings::join_path(config_path, "default")?;
                let env_file = Settings::join_path(config_path, &env)?;
                let local_file = Settings::join_path(config_path, "local")?;

                builder
                    // Start off by merging in the "default" configuration file
                    .add_source(File::with_name(&default_file).required(false))
                    // Add in the current environment file
                    // Default to 'development' env
                    // Note that this file is _optional_
                    .add_source(File::with_name(&env_file).required(false))
                    // Add in a local configuration file
                    // This file shouldn't be checked in to git
                    .add_source(File::with_name(&local_file).required(false))
            }
            None => builder,
        };

        let s = builder
            // Add in settings from the environment (with a prefix of KELLNR)
            .add_source(
                Environment::with_prefix("KELLNR")
                    .list_separator(",")
                    .with_list_parse_key("registry.required_crate_fields")
                    .try_parsing(true)
                    .prefix_separator("_")
                    .separator("__"),
            )
            .build()?;

        // You can deserialize (and thus freeze) the entire configuration as
        s.try_deserialize()
    }
}

impl Settings {
    fn join_path(config_path: &Path, file: &str) -> Result<String, ConfigError> {
        config_path
            .join(file)
            .to_str()
            .map(ToString::to_string)
            .ok_or_else(|| ConfigError::Message("Invalid UTF-8 string".to_string()))
    }

    pub fn bin_path(&self) -> PathBuf {
        PathBuf::from(&self.registry.data_dir).join("crates")
    }

    pub fn doc_queue_path(&self) -> PathBuf {
        PathBuf::from(&self.registry.data_dir).join("doc_queue")
    }

    pub fn sqlite_path(&self) -> PathBuf {
        PathBuf::from(&self.registry.data_dir).join("db.sqlite")
    }

    pub fn docs_path(&self) -> PathBuf {
        PathBuf::from(&self.registry.data_dir).join("docs")
    }

    pub fn base_path(&self) -> PathBuf {
        PathBuf::from(&self.registry.data_dir).join("git")
    }

    pub fn crates_io_bin_path(&self) -> PathBuf {
        PathBuf::from(&self.registry.data_dir).join("cratesio")
    }

    pub fn crates_io_path(&self) -> String {
        format!("{}/cratesio", self.registry.data_dir)
    }

    pub fn crates_path(&self) -> String {
        format!("{}/crates", self.registry.data_dir)
    }

    pub fn crates_path_or_bucket(&self) -> String {
        if self.s3.enabled
            && let Some(bucket) = &self.s3.crates_bucket
        {
            bucket.clone()
        } else {
            self.crates_path()
        }
    }

    pub fn crates_io_path_or_bucket(&self) -> String {
        if self.s3.enabled
            && let Some(bucket) = &self.s3.cratesio_bucket
        {
            bucket.clone()
        } else {
            self.crates_io_path()
        }
    }
}

pub fn get_settings() -> Result<Settings, ConfigError> {
    let path = if Path::new(compile_time_config::KELLNR_CONFIG_DIR).exists() {
        Some(Path::new(compile_time_config::KELLNR_CONFIG_DIR))
    } else if Path::new("./config").exists() {
        Some(Path::new("./config"))
    } else if Path::new("../config").exists() {
        Some(Path::new("../config"))
    } else if Path::new("../../config").exists() {
        Some(Path::new("../../config"))
    } else {
        None
    };

    Settings::try_from(path)
}

/// Used in Unit and Integration tests to provide test settings
pub fn test_settings() -> Settings {
    Settings {
        registry: Registry {
            data_dir: "/tmp/kdata_test".to_string(),
            ..Registry::default()
        },
        ..Settings::default()
    }
}
