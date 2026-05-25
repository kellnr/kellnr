use std::path::{Path, PathBuf};

/// Error returned by settings construction. Aliases [`provcfg::Error`] —
/// every failure surfaces from there (source deserialization or
/// config-file I/O), so kellnr-side code keeps a domain-flavoured name
/// without inventing its own opaque box.
pub use provcfg::Error as SettingsError;
use provcfg::{ClapArgs, Configurable, Provenance};
use serde::{Deserialize, Serialize};

use crate::config_source::SourceMap;
use crate::docs::{Docs, DocsArgs, DocsPartial, DocsProv};
use crate::local::{Local, LocalArgs, LocalPartial, LocalProv};
use crate::log::{Log, LogArgs, LogPartial, LogProv};
use crate::oauth2::{OAuth2, OAuth2Args, OAuth2Partial, OAuth2Prov};
use crate::origin::{Origin, OriginArgs, OriginPartial, OriginProv};
use crate::postgresql::{Postgresql, PostgresqlArgs, PostgresqlPartial, PostgresqlProv};
use crate::proxy::{Proxy, ProxyArgs, ProxyPartial, ProxyProv};
use crate::registry::{Registry, RegistryArgs, RegistryPartial, RegistryProv};
use crate::s3::{S3, S3Args, S3Partial, S3Prov};
use crate::setup::{Setup, SetupArgs, SetupPartial, SetupProv};
use crate::toolchain::{Toolchain, ToolchainArgs, ToolchainPartial, ToolchainProv};

/// Pure configuration values — every leaf carries the active value, no
/// provenance attached. Provenance lives on the companion `SettingsProv`,
/// which is built by [`build_prov_with_cli`] and stored in the app state for
/// the one handler (`/settings`) that needs it.
#[derive(Debug, Default, Deserialize, Serialize, Eq, PartialEq, Clone, Configurable, ClapArgs)]
#[serde(default)]
pub struct Settings {
    #[configurable(nested)]
    pub setup: Setup,
    #[configurable(nested)]
    pub registry: Registry,
    #[configurable(nested)]
    pub docs: Docs,
    #[configurable(nested)]
    pub proxy: Proxy,
    #[configurable(nested)]
    pub log: Log,
    #[configurable(nested)]
    pub local: Local,
    #[configurable(nested)]
    pub origin: Origin,
    #[configurable(nested)]
    pub postgresql: Postgresql,
    #[configurable(nested)]
    pub s3: S3,
    #[configurable(nested)]
    pub oauth2: OAuth2,
    #[configurable(nested)]
    pub toolchain: Toolchain,
}

/// Build a `SettingsProv` from the configured sources: optional TOML file,
/// `KELLNR_*` environment variables, and optionally a CLI partial.
///
/// Source order matters — later sources override earlier ones (TOML < env < CLI).
///
/// Exposed publicly (in addition to being called by [`crate::cli::parse_cli`])
/// so consumers like the `kellnr config show` printer can render per-leaf
/// provenance directly without duplicating the source-stack wiring.
pub fn build_prov_with_cli(
    config_file: Option<&Path>,
    cli_partial: Option<SettingsPartial>,
) -> Result<SettingsProv, SettingsError> {
    let mut cfg = provcfg::Config::new();
    if let Some(path) = config_file {
        cfg = cfg.add_toml_file(path)?;
    }
    cfg = cfg.add_env_with_list_keys("KELLNR", env_list_keys().iter().copied());
    if let Some(partial) = cli_partial {
        cfg = cfg.add_cli(partial);
    }
    cfg.build::<SettingsProv>()
}

/// Dotted paths that the env source should treat as comma-separated lists.
/// Matches the kellnr 6.x behaviour driven by the `config` crate's
/// `with_list_parse_key`.
pub(crate) fn env_list_keys() -> &'static [&'static str] {
    &["registry.required_crate_fields", "oauth2.scopes"]
}

/// Convert provcfg's per-leaf provenance map into kellnr's `SourceMap`.
/// Called by the `/settings` web-ui handler to build the API response.
pub fn sources_from_prov(prov: &SettingsProv) -> SourceMap {
    prov.sources_map()
        .into_iter()
        .map(|(key, category)| (key, category.into()))
        .collect()
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

/// Used in unit and integration tests to provide test settings.
pub fn test_settings() -> Settings {
    Settings {
        registry: Registry {
            data_dir: "/tmp/kdata_test".to_string(),
            ..Registry::default()
        },
        ..Settings::default()
    }
}

#[cfg(test)]
mod tests {
    use provcfg::Config;

    use super::*;
    use crate::config_source::ConfigSource;

    #[test]
    fn sources_from_prov_with_no_sources_marks_every_leaf_default() {
        let prov = Config::new().build::<SettingsProv>().unwrap();
        let map = sources_from_prov(&prov);

        // Spot-check a flat leaf and a nested one — every entry must be Default.
        assert_eq!(map.get("registry.data_dir"), Some(&ConfigSource::Default));
        assert_eq!(map.get("docs.enabled"), Some(&ConfigSource::Default));
        assert_eq!(map.get("s3.enabled"), Some(&ConfigSource::Default));
        assert!(
            map.values().all(|s| *s == ConfigSource::Default),
            "every entry must report Default when no sources were added"
        );
    }

    #[test]
    fn sources_from_prov_attributes_toml_leaves_to_toml_source() {
        let prov = Config::new()
            .add_toml_str(
                "test.toml",
                "[registry]\ndata_dir = \"/tmp/x\"\n[docs]\nenabled = true\n",
            )
            .build::<SettingsProv>()
            .unwrap();
        let map = sources_from_prov(&prov);

        assert_eq!(map.get("registry.data_dir"), Some(&ConfigSource::Toml));
        assert_eq!(map.get("docs.enabled"), Some(&ConfigSource::Toml));
        // Untouched leaves keep Default — File → Toml mapping must not bleed
        // across leaves that the TOML source didn't supply.
        assert_eq!(map.get("registry.cache_size"), Some(&ConfigSource::Default));
        assert_eq!(map.get("s3.enabled"), Some(&ConfigSource::Default));
    }
}
