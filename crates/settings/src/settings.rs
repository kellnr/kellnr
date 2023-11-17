use config::{Config, ConfigError, Environment, File};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::{
    convert::TryFrom,
    env,
    path::{self, Path},
};

use crate::docs::Docs;
use crate::local::Local;
use crate::log::Log;
use crate::origin::Origin;
use crate::postgresql::Postgresql;
use crate::proxy::Proxy;
use crate::registry::Registry;
use crate::setup::Setup;

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Default, Clone)]
pub struct Settings {
    pub setup: Setup,
    pub registry: Registry,
    pub docs: Docs,
    pub proxy: Proxy,
    pub log: Log,
    pub local: Local,
    pub origin: Origin,
    pub postgresql: Postgresql,
}

impl TryFrom<&Path> for Settings {
    type Error = ConfigError;

    fn try_from(config_path: &Path) -> Result<Self, Self::Error> {
        let default_file = Settings::join_path(config_path, "default")?;
        let env = env::var("RUN_MODE").unwrap_or_else(|_| "development".into());
        let env_file = Settings::join_path(config_path, &env)?;
        let local_file = Settings::join_path(config_path, "local")?;

        let s = Config::builder()
            // Start off by merging in the "default" configuration file
            .add_source(File::with_name(&default_file))
            // Add in the current environment file
            // Default to 'development' env
            // Note that this file is _optional_
            .add_source(File::with_name(&env_file).required(false))
            // Add in a local configuration file
            // This file shouldn't be checked in to git
            .add_source(File::with_name(&local_file).required(false))
            // Add in settings from the environment (with a prefix of KELLNR)
            // Eg. `KELLNR_DEBUG=1 ./target/app` would set the `debug` key
            .add_source(
                Environment::with_prefix("KELLNR")
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
            .map(|x| x.to_string())
            .ok_or_else(|| ConfigError::Message("Invalid UTF-8 string".to_string()))
    }

    pub fn bin_path(&self) -> path::PathBuf {
        path::PathBuf::from(&self.registry.data_dir).join("crates")
    }

    pub fn doc_queue_path(&self) -> PathBuf {
        path::PathBuf::from(&self.registry.data_dir).join("doc_queue")
    }

    pub fn sqlite_path(&self) -> path::PathBuf {
        path::PathBuf::from(&self.registry.data_dir).join("db.sqlite")
    }

    pub fn docs_path(&self) -> path::PathBuf {
        path::PathBuf::from(&self.registry.data_dir).join("docs")
    }

    pub fn base_path(&self) -> path::PathBuf {
        path::PathBuf::from(&self.registry.data_dir).join("git")
    }

    pub fn crates_io_bin_path(&self) -> path::PathBuf {
        path::PathBuf::from(&self.registry.data_dir).join("cratesio")
    }
}

pub fn get_settings() -> Result<Settings, ConfigError> {
    let path = if Path::new("./config").exists() {
        Path::new("./config")
    } else if Path::new("../config").exists() {
        Path::new("../config")
    } else if Path::new("../../config").exists() {
        Path::new("../../config")
    } else {
        return Err(ConfigError::NotFound("Config folder not found".to_string()));
    };

    Settings::try_from(path)
}
