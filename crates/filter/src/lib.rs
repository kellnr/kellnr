//! Filters that decide whether a crate may be downloaded, and which versions
//! the sparse index of the crates.io proxy serves.
//!
//! A [`FilterChain`] runs all configured [`CrateFilter`]s. To add a filter,
//! implement [`CrateFilter`] and add the data it needs to [`CrateInfo`]. The
//! license and the publish time are only looked up for a download if a filter
//! needs them, see [`CrateFilter::needs_license`] and
//! [`CrateFilter::needs_pubtime`].
//!
//! An [`IndexFilterChain`] runs all configured [`IndexFilter`]s on the
//! entries of an index file. A filter that hides versions from cargo
//! implements [`IndexFilter`], and [`CrateFilter`] too if their downloads must
//! be denied as well, like [`ReleaseAgeFilter`].

mod index;
mod known_licenses;
mod known_pubtimes;
mod license;
mod release_age;

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
pub use index::{IndexFilter, IndexFilterChain, IndexFilters};
use kellnr_common::original_name::OriginalName;
use kellnr_common::version::Version;
use kellnr_settings::{CratesioFilter, RegistryFilter, Settings};
pub use known_licenses::{KnownLicense, KnownLicenses};
pub use known_pubtimes::{KnownPubtime, KnownPubtimes};
pub use license::LicenseFilter;
pub use release_age::ReleaseAgeFilter;

/// The data of a crate version the filters decide on.
pub struct CrateInfo<'a> {
    pub name: &'a OriginalName,
    pub version: &'a Version,
    /// `None` if the license could not be determined. It may stay `None` if
    /// no filter needs it, see [`CrateFilter::needs_license`].
    pub license: Option<&'a str>,
    /// When the version was published on crates.io, from the `pubtime` of its
    /// index entry. `None` if the entry has none, and it may stay `None` if
    /// no filter needs it, see [`CrateFilter::needs_pubtime`].
    pub pubtime: Option<DateTime<Utc>>,
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
                krate.name.to_lowercase(),
                krate.version
            )),
        }
    }
}

pub trait CrateFilter: Send + Sync {
    fn name(&self) -> &'static str;
    fn check(&self, krate: &CrateInfo) -> Verdict;

    /// Whether the filter needs [`CrateInfo::license`] to check versions of
    /// the crate `name`.
    fn needs_license(&self, _name: &OriginalName) -> bool {
        false
    }

    /// Whether the filter needs [`CrateInfo::pubtime`] to check versions of
    /// the crate `name`.
    fn needs_pubtime(&self, _name: &OriginalName) -> bool {
        false
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FilterError {
    #[error("Invalid license \"{license}\" in {section}.license_allowed: {reason}")]
    InvalidLicense {
        section: &'static str,
        license: String,
        reason: String,
    },
    #[error("Invalid crate name \"{name}\" in {setting}: {reason}")]
    InvalidCrateName {
        setting: &'static str,
        name: String,
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
    licenses: KnownLicenses,
    pubtimes: KnownPubtimes,
}

impl FilterChain {
    pub fn new(filters: Vec<Arc<dyn CrateFilter>>) -> Self {
        Self {
            filters,
            licenses: KnownLicenses::default(),
            pubtimes: KnownPubtimes::default(),
        }
    }

    /// Licenses looked up for this chain, so that downloads do not need to
    /// query them again.
    pub fn licenses(&self) -> &KnownLicenses {
        &self.licenses
    }

    /// Publish times looked up for this chain, so that downloads do not need
    /// to query them again.
    pub fn pubtimes(&self) -> &KnownPubtimes {
        &self.pubtimes
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
        // After the license filter, as its denial is permanent, which is the
        // more useful message.
        if settings.min_release_age_seconds > 0 {
            filters.push(Arc::new(ReleaseAgeFilter::new(
                Duration::from_secs(settings.min_release_age_seconds),
                &settings.min_release_age_exempt_crates,
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

    /// Whether a filter needs [`CrateInfo::license`] to check versions of the
    /// crate `name`.
    pub fn needs_license(&self, name: &OriginalName) -> bool {
        self.filters.iter().any(|f| f.needs_license(name))
    }

    /// Whether a filter needs [`CrateInfo::pubtime`] to check versions of the
    /// crate `name`.
    pub fn needs_pubtime(&self, name: &OriginalName) -> bool {
        self.filters.iter().any(|f| f.needs_pubtime(name))
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
            name: &OriginalName::try_from("foo").unwrap(),
            version: &Version::try_from("1.2.3").unwrap(),
            license: None,
            pubtime: None,
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
            name: &OriginalName::try_from("foo").unwrap(),
            version: &Version::try_from("1.2.3").unwrap(),
            license: Some("GPL-3.0"),
            pubtime: None,
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

    fn release_age(exempt: &[&str]) -> CratesioFilter {
        CratesioFilter {
            min_release_age_seconds: 3 * 86_400,
            min_release_age_exempt_crates: exempt.iter().map(ToString::to_string).collect(),
            ..CratesioFilter::default()
        }
    }

    #[test]
    fn chain_with_min_release_age_needs_the_publish_time_of_crates_not_exempt() {
        let chain = FilterChain::cratesio(&release_age(&["openssl"])).unwrap();

        assert!(!chain.is_empty());
        assert!(chain.needs_pubtime(&OriginalName::try_from("serde").unwrap()));
        assert!(!chain.needs_pubtime(&OriginalName::try_from("openssl").unwrap()));
    }

    #[test]
    fn chains_without_min_release_age_need_no_publish_time() {
        let license_only = FilterChain::cratesio(&CratesioFilter {
            license_enabled: true,
            license_allowed: vec!["MIT".to_string()],
            ..CratesioFilter::default()
        })
        .unwrap();
        let serde = OriginalName::try_from("serde").unwrap();

        assert!(!license_only.needs_pubtime(&serde));
        assert!(!FilterChain::default().needs_pubtime(&serde));
    }

    #[test]
    fn only_the_license_filter_needs_the_license_of_crates_not_exempt() {
        let license = FilterChain::cratesio(&CratesioFilter {
            license_enabled: true,
            license_allowed: vec!["MIT".to_string()],
            license_exempt_crates: vec!["openssl".to_string()],
            ..CratesioFilter::default()
        })
        .unwrap();
        let release_age_only = FilterChain::cratesio(&release_age(&[])).unwrap();
        let serde = OriginalName::try_from("serde").unwrap();

        assert!(license.needs_license(&serde));
        assert!(!license.needs_license(&OriginalName::try_from("OpenSSL").unwrap()));
        assert!(!release_age_only.needs_license(&serde));
        assert!(!FilterChain::default().needs_license(&serde));
    }

    #[test]
    fn license_denial_comes_before_the_release_age() {
        let chain = FilterChain::cratesio(&CratesioFilter {
            license_enabled: true,
            license_allowed: vec!["MIT".to_string()],
            ..release_age(&[])
        })
        .unwrap();

        let verdict = chain.check(&CrateInfo {
            name: &OriginalName::try_from("foo").unwrap(),
            version: &Version::try_from("1.2.3").unwrap(),
            license: Some("GPL-3.0-only"),
            pubtime: Some(Utc::now()),
        });

        let Verdict::Deny { filter, .. } = verdict else {
            panic!("the crate must be denied");
        };
        assert_eq!("license", filter);
    }

    #[test]
    fn invalid_min_release_age_exempt_crate_is_an_error() {
        let Err(e) = FilterChain::cratesio(&release_age(&["open ssl"])) else {
            panic!("an invalid crate name must be rejected");
        };

        assert!(
            e.to_string()
                .contains("cratesio_filter.min_release_age_exempt_crates")
        );
        assert!(e.to_string().contains("open ssl"));
    }
}
