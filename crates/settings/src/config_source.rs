use std::collections::HashMap;

use provcfg::Category;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Represents the source of a configuration value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ConfigSource {
    /// Value comes from compiled-in defaults
    Default,
    /// Value was set in a TOML configuration file
    Toml,
    /// Value was set via environment variable
    Env,
    /// Value was set via command-line argument
    Cli,
}

/// Map provcfg's source categories onto kellnr's API-facing enum. kellnr only
/// loads TOML files, so `Category::File` becomes `Toml`; `Category::Default`
/// plus any future provcfg variants fall to `Default` via the wildcard arm.
impl From<Category> for ConfigSource {
    fn from(category: Category) -> Self {
        match category {
            Category::File => Self::Toml,
            Category::Env => Self::Env,
            Category::Cli => Self::Cli,
            _ => Self::Default,
        }
    }
}

/// Maps setting keys (e.g., `"registry.data_dir"`) to their configuration source.
///
/// Built on demand by [`crate::settings::sources_from_prov`] from the
/// `SettingsProv` produced by [`crate::settings::build_prov_with_cli`]. Keys
/// not present in the map are treated as `Default` by every reader (see
/// `crates/kellnr/src/config_printer.rs`), so it is not pre-populated.
pub type SourceMap = HashMap<String, ConfigSource>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_source_serialization() {
        assert_eq!(
            serde_json::to_string(&ConfigSource::Default).unwrap(),
            "\"default\""
        );
        assert_eq!(
            serde_json::to_string(&ConfigSource::Toml).unwrap(),
            "\"toml\""
        );
        assert_eq!(
            serde_json::to_string(&ConfigSource::Env).unwrap(),
            "\"env\""
        );
        assert_eq!(
            serde_json::to_string(&ConfigSource::Cli).unwrap(),
            "\"cli\""
        );
    }

    #[test]
    fn from_category_maps_file_env_cli_explicitly() {
        assert_eq!(ConfigSource::from(Category::File), ConfigSource::Toml);
        assert_eq!(ConfigSource::from(Category::Env), ConfigSource::Env);
        assert_eq!(ConfigSource::from(Category::Cli), ConfigSource::Cli);
    }

    #[test]
    fn from_category_falls_back_to_default_for_default_category() {
        // The wildcard arm also catches any future `Category` variants provcfg
        // may add, guarding the kellnr API against silent breakage.
        assert_eq!(ConfigSource::from(Category::Default), ConfigSource::Default);
    }
}
