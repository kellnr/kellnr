use std::convert::TryFrom;
use std::path::{Path, PathBuf};

use config::{Config, ConfigError, Environment, File};
use serde::{Deserialize, Serialize};

use crate::docs::Docs;
use crate::local::Local;
use crate::log::Log;
use crate::oauth2::OAuth2;
use crate::origin::Origin;
use crate::postgresql::Postgresql;
use crate::proxy::Proxy;
use crate::registry::Registry;
use crate::s3::S3;
use crate::setup::Setup;
use crate::toolchain::Toolchain;

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
    pub oauth2: OAuth2,
    pub toolchain: Toolchain,
}

impl TryFrom<Option<&Path>> for Settings {
    type Error = ConfigError;

    fn try_from(config_file: Option<&Path>) -> Result<Self, Self::Error> {
        let mut builder = Config::builder();

        // Add configuration from file if provided
        if let Some(path) = config_file {
            builder = builder.add_source(File::from(path).required(true));
        }

        // Add settings from environment variables (with prefix KELLNR)
        builder = builder.add_source(
            Environment::with_prefix("KELLNR")
                .list_separator(",")
                .with_list_parse_key("registry.required_crate_fields")
                .with_list_parse_key("oauth2.scopes")
                .try_parsing(true)
                .prefix_separator("_")
                .separator("__"),
        );

        builder.build()?.try_deserialize()
    }
}

impl Settings {
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
        if self.s3.enabled {
            self.s3.crates_bucket.clone()
        } else {
            self.crates_path()
        }
    }

    pub fn crates_io_path_or_bucket(&self) -> String {
        if self.s3.enabled {
            self.s3.cratesio_bucket.clone()
        } else {
            self.crates_io_path()
        }
    }

    pub fn toolchain_path(&self) -> String {
        format!("{}/toolchains", self.registry.data_dir)
    }

    pub fn toolchain_path_or_bucket(&self) -> String {
        if self.s3.enabled {
            self.s3.toolchain_bucket.clone()
        } else {
            self.toolchain_path()
        }
    }
}

pub fn get_settings() -> Result<Settings, ConfigError> {
    let path = crate::compile_time_config::KELLNR_COMPTIME__CONFIG_FILE.map(Path::new);
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
