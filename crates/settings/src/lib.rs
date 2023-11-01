use config::{Config, ConfigError, Environment, File};
use deserialize_with::DeserializeWith;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::{
    convert::TryFrom,
    env,
    net::IpAddr,
    path::{self, Path},
};

pub mod constants;
mod deserialize_with;
mod log_format;
mod log_level;
mod protocol;

pub use log_format::LogFormat;
pub use protocol::Protocol;

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Default, Clone)]
pub struct Postgresql {
    pub enabled: bool,
    pub address: String,
    pub port: u16,
    pub db: String,
    pub user: String,
    #[serde(skip_serializing, default)]
    pub pwd: String,
}

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub admin_pwd: String,
    pub data_dir: String,
    pub session_age_seconds: u64,
    pub api_address: String,
    pub api_port: u16,
    pub api_port_proxy: u16,
    #[serde(
        deserialize_with = "Protocol::deserialize_with",
        default = "Protocol::default"
    )]
    pub api_protocol: Protocol,
    pub index_address: String,
    pub web_address: IpAddr,
    pub index_port: u16,
    pub admin_token: String,
    #[serde(default)]
    pub crates_io_proxy: bool,
    pub crates_io_num_threads: usize,
    #[serde(deserialize_with = "tracing::Level::deserialize_with")]
    pub log_level: tracing::Level,
    #[serde(deserialize_with = "tracing::Level::deserialize_with")]
    pub log_level_web_server: tracing::Level,
    #[serde(deserialize_with = "LogFormat::deserialize_with")]
    pub log_format: LogFormat,
    #[serde(default)]
    pub postgresql: Postgresql,
    pub rustdoc_auto_gen: bool,
    #[serde(default)]
    pub cache_size: u64,
    pub max_crate_size: usize,
    pub max_docs_size: usize,
    #[serde(default)]
    pub auth_required: bool,
}

impl TryFrom<&Path> for Settings {
    type Error = ConfigError;

    fn try_from(config_path: &Path) -> Result<Self, Self::Error> {
        let default_file = Settings::join_path(config_path, "default")?;
        let env = env::var("RUN_MODE").unwrap_or_else(|_| "development".into());
        let env_file = Settings::join_path(config_path, &env)?;
        let local_file = Settings::join_path(config_path, "local")?;

        let s = Config::builder()
            // Set default values of settings that where added after the first Kellnr release
            // and thus can be missing on the customer site.
            .set_default("log_level", "info")?
            .set_default("log_level_web_server", "warn")?
            .set_default("log_format", "compact")?
            .set_default("api_port_proxy", 8000)?
            .set_default("rustdoc_auto_gen", true)?
            .set_default("max_crate_size", 100 * 1000)?
            .set_default("max_docs_size", 100 * 1000)?
            .set_default("crates_io_num_threads", 10)?
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
    pub fn new() -> Result<Self, ConfigError> {
        Settings::try_from(Path::new("../../config"))
    }

    fn join_path(config_path: &Path, file: &str) -> Result<String, ConfigError> {
        config_path
            .join(file)
            .to_str()
            .map(|x| x.to_string())
            .ok_or_else(|| ConfigError::Message("Invalid UTF-8 string".to_string()))
    }

    pub fn bin_path(&self) -> path::PathBuf {
        path::PathBuf::from(&self.data_dir).join("crates")
    }

    pub fn doc_queue_path(&self) -> PathBuf {
        path::PathBuf::from(&self.data_dir).join("doc_queue")
    }

    pub fn sqlite_path(&self) -> path::PathBuf {
        path::PathBuf::from(&self.data_dir).join("db.sqlite")
    }

    pub fn docs_path(&self) -> path::PathBuf {
        path::PathBuf::from(&self.data_dir).join("docs")
    }

    pub fn base_path(&self) -> path::PathBuf {
        path::PathBuf::from(&self.data_dir).join("git")
    }

    pub fn crates_io_bin_path(&self) -> path::PathBuf {
        path::PathBuf::from(&self.data_dir).join("cratesio")
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
