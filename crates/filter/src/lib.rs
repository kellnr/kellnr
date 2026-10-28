//! Filters that decide whether a crate may be downloaded.
//!
//! A [`FilterChain`] runs all configured [`CrateFilter`]s. To add a filter,
//! implement [`CrateFilter`] and add the data it needs to [`CrateInfo`].

mod license;

use std::sync::Arc;

use kellnr_common::normalized_name::NormalizedName;
use kellnr_common::version::Version;
use kellnr_settings::{CratesioFilter, RegistryFilter, Settings};
pub use license::LicenseFilter;

/// The data of a crate version the filters decide on.
pub struct CrateInfo<'a> {
    pub name: &'a NormalizedName,
    pub version: &'a Version,
    /// `None` if the license could not be determined.
    pub license: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Allow,
    Deny {
        filter: &'static str,
        reason: String,
    },
}

impl Verdict {
    /// Message for the client if the download is denied.
    pub fn deny_message(&self, krate: &CrateInfo) -> Option<String> {
        match self {
            Verdict::Allow => None,
            Verdict::Deny { filter, reason } => Some(format!(
                "Download of {}@{} denied by Kellnr {filter} filter: {reason}",
                krate.name, krate.version
            )),
        }
    }
}

pub trait CrateFilter: Send + Sync {
    fn name(&self) -> &'static str;
    fn check(&self, krate: &CrateInfo) -> Verdict;
}

#[derive(Debug, thiserror::Error)]
pub enum FilterError {
    #[error("Invalid license \"{license}\" in {section}.license_allowed: {reason}")]
    InvalidLicense {
        section: &'static str,
        license: String,
        reason: String,
    },
}

/// The filters for both download paths.
#[derive(Clone, Default)]
pub struct DownloadFilters {
    pub cratesio: FilterChain,
    pub registry: FilterChain,
}

impl DownloadFilters {
    pub fn from_settings(settings: &Settings) -> Result<Self, FilterError> {
        Ok(Self {
            cratesio: FilterChain::cratesio(&settings.cratesio_filter)?,
            registry: FilterChain::registry(&settings.registry_filter)?,
        })
    }
}

/// Runs all filters, the first `Deny` wins.
#[derive(Clone, Default)]
pub struct FilterChain {
    filters: Vec<Arc<dyn CrateFilter>>,
}

impl FilterChain {
    pub fn new(filters: Vec<Arc<dyn CrateFilter>>) -> Self {
        Self { filters }
    }

    /// Filters for downloads through the crates.io proxy.
    pub fn cratesio(settings: &CratesioFilter) -> Result<Self, FilterError> {
        let mut filters: Vec<Arc<dyn CrateFilter>> = Vec::new();
        if settings.license_enabled {
            filters.push(Arc::new(LicenseFilter::new(
                "cratesio_filter",
                &settings.license_allowed,
                settings.license_unknown,
                &settings.license_exempt_crates,
            )?));
        }
        Ok(Self::new(filters))
    }

    /// Filters for downloads of crates hosted in Kellnr.
    pub fn registry(settings: &RegistryFilter) -> Result<Self, FilterError> {
        let mut filters: Vec<Arc<dyn CrateFilter>> = Vec::new();
        if settings.license_enabled {
            filters.push(Arc::new(LicenseFilter::new(
                "registry_filter",
                &settings.license_allowed,
                settings.license_unknown,
                &settings.license_exempt_crates,
            )?));
        }
        Ok(Self::new(filters))
    }

    pub fn is_empty(&self) -> bool {
        self.filters.is_empty()
    }

    pub fn check(&self, krate: &CrateInfo) -> Verdict {
        self.filters
            .iter()
            .map(|f| f.check(krate))
            .find(|v| matches!(v, Verdict::Deny { .. }))
            .unwrap_or(Verdict::Allow)
    }
}

#[cfg(test)]
mod tests {
    use kellnr_settings::LicenseUnknown;

    use super::*;

    struct Fixed(&'static str, Verdict);

    impl CrateFilter for Fixed {
        fn name(&self) -> &'static str {
            self.0
        }

        fn check(&self, _krate: &CrateInfo) -> Verdict {
            self.1.clone()
        }
    }

    fn deny(filter: &'static str) -> Verdict {
        Verdict::Deny {
            filter,
            reason: "no".to_string(),
        }
    }

    fn check(chain: &FilterChain) -> Verdict {
        chain.check(&CrateInfo {
            name: &NormalizedName::from_unchecked_str("foo"),
            version: &Version::from_unchecked_str("1.2.3"),
            license: None,
        })
    }

    #[test]
    fn empty_chain_allows() {
        assert!(FilterChain::default().is_empty());
        assert_eq!(Verdict::Allow, check(&FilterChain::default()));
    }

    #[test]
    fn first_deny_wins() {
        let chain = FilterChain::new(vec![
            Arc::new(Fixed("a", Verdict::Allow)),
            Arc::new(Fixed("b", deny("b"))),
            Arc::new(Fixed("c", deny("c"))),
        ]);

        assert_eq!(deny("b"), check(&chain));
    }

    #[test]
    fn deny_message_names_crate_and_filter() {
        let krate = CrateInfo {
            name: &NormalizedName::from_unchecked_str("foo"),
            version: &Version::from_unchecked_str("1.2.3"),
            license: Some("GPL-3.0"),
        };
        let verdict = Verdict::Deny {
            filter: "license",
            reason: "license \"GPL-3.0\" is not allowed".to_string(),
        };

        assert_eq!(
            Some(
                "Download of foo@1.2.3 denied by Kellnr license filter: license \"GPL-3.0\" is not allowed"
                    .to_string()
            ),
            verdict.deny_message(&krate)
        );
        assert_eq!(None, Verdict::Allow.deny_message(&krate));
    }

    #[test]
    fn chain_from_disabled_settings_is_empty() {
        let chain = FilterChain::cratesio(&CratesioFilter::default()).unwrap();

        assert!(chain.is_empty());
    }

    #[test]
    fn chain_from_enabled_settings_has_license_filter() {
        let chain = FilterChain::registry(&RegistryFilter {
            license_enabled: true,
            license_allowed: vec!["MIT".to_string()],
            license_unknown: LicenseUnknown::Deny,
            license_exempt_crates: vec![],
        })
        .unwrap();

        assert!(!chain.is_empty());
    }

    #[test]
    fn invalid_allowed_license_is_an_error() {
        let result = FilterChain::cratesio(&CratesioFilter {
            license_enabled: true,
            license_allowed: vec!["MTI".to_string()],
            ..CratesioFilter::default()
        });

        let Err(e) = result else {
            panic!("an invalid license must be rejected");
        };
        assert!(e.to_string().contains("cratesio_filter.license_allowed"));
        assert!(e.to_string().contains("MTI"));
    }
}
