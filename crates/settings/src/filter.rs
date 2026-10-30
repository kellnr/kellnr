// The field names are the settings keys. The prefix leaves room for other filters.
#![allow(clippy::struct_field_names)]

use std::fmt::Display;

use clap::ValueEnum;
use provcfg::{ClapArgs, Configurable};
use serde::{Deserialize, Deserializer, Serialize};

/// Filters for downloads through the crates.io proxy.
///
/// The fields are the same as in [`RegistryFilter`]. Both are separate structs
/// because the CLI flag prefix is set per struct.
#[derive(Debug, Default, Deserialize, Serialize, Eq, PartialEq, Clone, Configurable, ClapArgs)]
#[serde(default)]
#[configurable(clap_prefix = "cratesio-filter")]
pub struct CratesioFilter {
    /// Deny downloads of crates whose license is not allowed
    pub license_enabled: bool,

    /// Allowed SPDX license identifiers (comma-separated), e.g. "MIT,Apache-2.0"
    #[configurable(env_list)]
    #[arg(value_delimiter = ',')]
    pub license_allowed: Vec<String>,

    /// Decision for crates without a known or valid license: "deny" or "allow".
    /// A license that is no valid SPDX expression counts as unknown
    #[arg(value_enum)]
    pub license_unknown: LicenseUnknown,

    /// Crates (comma-separated) that are allowed regardless of their license
    #[configurable(env_list)]
    #[arg(value_delimiter = ',')]
    pub license_exempt_crates: Vec<String>,
}

/// Filters for downloads of crates hosted in Kellnr.
///
/// The fields are the same as in [`CratesioFilter`].
#[derive(Debug, Default, Deserialize, Serialize, Eq, PartialEq, Clone, Configurable, ClapArgs)]
#[serde(default)]
#[configurable(clap_prefix = "registry-filter")]
pub struct RegistryFilter {
    /// Deny downloads of crates whose license is not allowed
    pub license_enabled: bool,

    /// Allowed SPDX license identifiers (comma-separated), e.g. "MIT,Apache-2.0"
    #[configurable(env_list)]
    #[arg(value_delimiter = ',')]
    pub license_allowed: Vec<String>,

    /// Decision for crates without a known or valid license: "deny" or "allow".
    /// A license that is no valid SPDX expression counts as unknown
    #[arg(value_enum)]
    pub license_unknown: LicenseUnknown,

    /// Crates (comma-separated) that are allowed regardless of their license
    #[configurable(env_list)]
    #[arg(value_delimiter = ',')]
    pub license_exempt_crates: Vec<String>,
}

/// What the license filter does with a crate whose license is unknown: none
/// is declared, the expression does not parse, or it could not be fetched.
#[derive(Debug, PartialEq, Eq, Clone, Copy, ValueEnum, Default)]
pub enum LicenseUnknown {
    #[default]
    Deny,
    Allow,
}

impl<'de> Deserialize<'de> for LicenseUnknown {
    fn deserialize<D>(de: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match String::deserialize(de)?.to_lowercase().as_str() {
            "deny" => Ok(Self::Deny),
            "allow" => Ok(Self::Allow),
            other => Err(serde::de::Error::custom(format!(
                "unknown license_unknown value {other:?}; expected \"deny\" or \"allow\""
            ))),
        }
    }
}

impl Serialize for LicenseUnknown {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl Display for LicenseUnknown {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LicenseUnknown::Deny => write!(f, "deny"),
            LicenseUnknown::Allow => write!(f, "allow"),
        }
    }
}

#[cfg(test)]
mod tests {
    use provcfg::Config;

    use crate::settings::SettingsProv;
    use crate::{CratesioFilter, LicenseUnknown, RegistryFilter, Settings};

    fn from_toml(toml: &str) -> Result<Settings, provcfg::Error> {
        Config::new()
            .add_toml_str("test.toml", toml)
            .build::<SettingsProv>()
            .map(|prov| Settings::from(&prov))
    }

    #[test]
    fn filters_are_disabled_by_default() {
        let settings = Settings::default();

        assert_eq!(CratesioFilter::default(), settings.cratesio_filter);
        assert_eq!(RegistryFilter::default(), settings.registry_filter);
        assert!(!settings.cratesio_filter.license_enabled);
        assert!(settings.cratesio_filter.license_allowed.is_empty());
        assert_eq!(
            LicenseUnknown::Deny,
            settings.cratesio_filter.license_unknown
        );
        assert!(settings.cratesio_filter.license_exempt_crates.is_empty());
    }

    #[test]
    fn both_sections_read_from_toml() {
        let settings = from_toml(
            r#"
            [cratesio_filter]
            license_enabled = true
            license_allowed = ["MIT", "Apache-2.0"]
            license_unknown = "allow"
            license_exempt_crates = ["ring"]

            [registry_filter]
            license_enabled = true
            license_allowed = ["MIT"]
            "#,
        )
        .unwrap();

        assert_eq!(
            CratesioFilter {
                license_enabled: true,
                license_allowed: vec!["MIT".to_string(), "Apache-2.0".to_string()],
                license_unknown: LicenseUnknown::Allow,
                license_exempt_crates: vec!["ring".to_string()],
            },
            settings.cratesio_filter
        );
        assert_eq!(
            RegistryFilter {
                license_enabled: true,
                license_allowed: vec!["MIT".to_string()],
                license_unknown: LicenseUnknown::Deny,
                license_exempt_crates: vec![],
            },
            settings.registry_filter
        );
    }

    #[test]
    fn invalid_license_unknown_fails() {
        let result = from_toml("[cratesio_filter]\nlicense_unknown = \"deneay\"\n");

        assert!(result.is_err());
    }

    #[test]
    fn list_settings_are_env_list_keys() {
        let keys = crate::settings::env_list_keys();

        for key in [
            "cratesio_filter.license_allowed",
            "cratesio_filter.license_exempt_crates",
            "registry_filter.license_allowed",
            "registry_filter.license_exempt_crates",
        ] {
            assert!(keys.contains(&key), "{key} must be an env list key");
        }
    }
}
