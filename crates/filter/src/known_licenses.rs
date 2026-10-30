use std::time::Duration;

use kellnr_common::canonical_name::CanonicalName;
use kellnr_common::version::Version;
use moka::sync::Cache;

const MAX_ENTRIES: u64 = 100_000;
/// Limits how long a Kellnr instance serves a stale license if another
/// instance deletes and re-publishes the version.
const TIME_TO_LIVE: Duration = Duration::from_hours(1);

/// The license a crate version declares, once it is known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KnownLicense {
    Declared(String),
    /// The version declares no license.
    Undeclared,
}

impl KnownLicense {
    pub fn into_license(self) -> Option<String> {
        match self {
            Self::Declared(license) => Some(license),
            Self::Undeclared => None,
        }
    }
}

impl From<Option<String>> for KnownLicense {
    fn from(license: Option<String>) -> Self {
        license.map_or(Self::Undeclared, Self::Declared)
    }
}

/// Licenses of crate versions that are known, so that a download needs no
/// database query. The license of a published version does not change.
#[derive(Clone)]
pub struct KnownLicenses(Cache<(CanonicalName, Version), KnownLicense>);

impl Default for KnownLicenses {
    fn default() -> Self {
        Self(
            Cache::builder()
                .max_capacity(MAX_ENTRIES)
                .time_to_live(TIME_TO_LIVE)
                .build(),
        )
    }
}

impl KnownLicenses {
    pub fn get(&self, name: &CanonicalName, version: &Version) -> Option<KnownLicense> {
        self.0.get(&(name.clone(), version.clone()))
    }

    pub fn insert(&self, name: &CanonicalName, version: &Version, license: KnownLicense) {
        self.0.insert((name.clone(), version.clone()), license);
    }

    /// Forget the license of a deleted version, as it may be published again.
    pub fn forget(&self, name: &CanonicalName, version: &Version) {
        self.0.invalidate(&(name.clone(), version.clone()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> (CanonicalName, Version) {
        (
            CanonicalName::try_from("foo").unwrap(),
            Version::from_unchecked_str("1.0.0"),
        )
    }

    #[test]
    fn remembers_licenses_and_declared_no_license() {
        let known = KnownLicenses::default();
        let (name, version) = key();
        let other = Version::from_unchecked_str("2.0.0");

        known.insert(&name, &version, KnownLicense::Declared("MIT".to_string()));
        known.insert(&name, &other, KnownLicense::Undeclared);

        assert_eq!(
            Some(KnownLicense::Declared("MIT".to_string())),
            known.get(&name, &version)
        );
        assert_eq!(Some(KnownLicense::Undeclared), known.get(&name, &other));
        assert_eq!(
            None,
            known.get(&name, &Version::from_unchecked_str("3.0.0"))
        );
    }

    #[test]
    fn forgets_deleted_versions() {
        let known = KnownLicenses::default();
        let (name, version) = key();
        known.insert(&name, &version, KnownLicense::Declared("MIT".to_string()));

        known.forget(&name, &version);

        assert_eq!(None, known.get(&name, &version));
    }
}
