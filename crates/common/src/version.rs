use std::convert::TryFrom;
use std::fmt;
use std::str::FromStr;

use sea_orm::Value;
use thiserror::Error;
use utoipa::ToSchema;

/// A crate version. It is always a valid `SemVer` version and is compared,
/// ordered and hashed like one, including build metadata. It is displayed,
/// serialized and stored as the version string it was parsed from.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, ToSchema)]
#[schema(value_type = String)]
pub struct Version(semver::Version);

impl serde::Serialize for Version {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(&self.0)
    }
}

impl<'de> serde::Deserialize<'de> for Version {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = <String as serde::Deserialize>::deserialize(deserializer)?;
        Version::try_from(s.as_str()).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Eq, PartialEq, Error)]
pub enum VersionError {
    #[error("Invalid SemVer")]
    InvalidSemVer,
}

impl Version {
    /// Whether both versions are equal when build metadata is ignored. Cargo
    /// does not distinguish `1.0.0+a` from `1.0.0+b`, so a registry must not
    /// host both.
    pub fn eq_ignoring_build(&self, other: &Version) -> bool {
        let (a, b) = (&self.0, &other.0);
        (a.major, a.minor, a.patch, &a.pre) == (b.major, b.minor, b.patch, &b.pre)
    }
}

impl From<Version> for Value {
    fn from(value: Version) -> Self {
        Value::String(Some(value.to_string()))
    }
}

impl From<&Version> for Value {
    fn from(value: &Version) -> Self {
        Value::String(Some(value.to_string()))
    }
}

impl TryFrom<&str> for Version {
    type Error = VersionError;

    fn try_from(version: &str) -> Result<Self, Self::Error> {
        semver::Version::parse(version)
            .map(Version)
            .map_err(|_| VersionError::InvalidSemVer)
    }
}

impl TryFrom<&String> for Version {
    type Error = VersionError;

    fn try_from(version: &String) -> Result<Self, Self::Error> {
        Version::try_from(version.as_str())
    }
}

impl TryFrom<String> for Version {
    type Error = VersionError;

    fn try_from(version: String) -> Result<Self, Self::Error> {
        Version::try_from(version.as_str())
    }
}

impl FromStr for Version {
    type Err = VersionError;

    fn from_str(version: &str) -> Result<Self, Self::Err> {
        Version::try_from(version)
    }
}

impl AsRef<Version> for Version {
    fn as_ref(&self) -> &Version {
        self
    }
}

impl Default for Version {
    fn default() -> Self {
        Version(semver::Version::new(0, 0, 0))
    }
}

impl fmt::Debug for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Version").field(&self.to_string()).finish()
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

/// The latest of `versions`. Stable versions win over pre-releases, like on
/// crates.io, so a pre-release is only the latest if there is no stable one.
/// Versions that cannot be parsed are ignored.
pub fn latest_version<'a>(versions: impl IntoIterator<Item = &'a str>) -> Option<String> {
    versions
        .into_iter()
        .filter_map(|v| semver::Version::parse(v).ok())
        .max_by(|a, b| (a.pre.is_empty(), a).cmp(&(b.pre.is_empty(), b)))
        .map(|v| v.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latest_version_prefers_stable_versions() {
        assert_eq!(
            Some("2.0.0".to_string()),
            latest_version(["1.0.0", "2.0.0", "1.5.0", "3.0.0-beta.1"])
        );
        assert_eq!(
            Some("3.0.0-beta.2".to_string()),
            latest_version(["3.0.0-beta.1", "3.0.0-beta.2"])
        );
        assert_eq!(
            Some("1.0.0".to_string()),
            latest_version(["1.0.0", "invalid"])
        );
        assert_eq!(None, latest_version([]));
    }

    #[test]
    fn valid_package_versions() {
        assert_eq!(Version::try_from("0.0.0").unwrap().to_string(), "0.0.0");
        assert_eq!(Version::try_from("1.0.1").unwrap().to_string(), "1.0.1");
        assert_eq!(
            Version::try_from("23.123.343").unwrap().to_string(),
            "23.123.343"
        );
        assert_eq!(
            Version::try_from("2.43.3-alpha34").unwrap().to_string(),
            "2.43.3-alpha34"
        );
        assert_eq!(
            Version::try_from("0.1.1-45rdfsd-45").unwrap().to_string(),
            "0.1.1-45rdfsd-45"
        );
    }

    #[test]
    fn eq_ignoring_build_ignores_only_build_metadata() {
        let v = |s: &str| Version::try_from(s).unwrap();

        assert!(v("1.0.0+a").eq_ignoring_build(&v("1.0.0+b")));
        assert!(v("1.0.0").eq_ignoring_build(&v("1.0.0+b")));
        assert!(v("1.0.0-rc.1+a").eq_ignoring_build(&v("1.0.0-rc.1")));
        assert!(!v("1.0.0-rc.1").eq_ignoring_build(&v("1.0.0")));
        assert!(!v("1.0.1+a").eq_ignoring_build(&v("1.0.0+a")));
    }

    #[test]
    fn invalid_package_versions() {
        assert_eq!(
            Version::try_from("a.1.2").unwrap_err(),
            VersionError::InvalidSemVer
        );
        assert_eq!(
            Version::try_from("002.23.1").unwrap_err(),
            VersionError::InvalidSemVer
        );
        assert_eq!(
            Version::try_from("3.2fg.3").unwrap_err(),
            VersionError::InvalidSemVer
        );
        assert_eq!(
            Version::try_from("5.3.2.3").unwrap_err(),
            VersionError::InvalidSemVer
        );
    }

    #[test]
    fn versions_are_ordered_like_semver() {
        let v = |s: &str| Version::try_from(s).unwrap();

        assert!(v("1.0.0-alpha") < v("1.0.0-alpha.1"));
        assert!(v("1.0.0-alpha.1") < v("1.0.0-beta"));
        assert!(v("1.0.0-rc.1") < v("1.0.0"));
        assert!(v("1.0.0") < v("1.0.1"));
        assert!(v("1.9.0") < v("1.10.0"));
        assert!(v("1.0.0") < v("1.0.0+build"));
        assert!(v("1.0.0+a") < v("1.0.0+b"));
    }

    #[test]
    fn versions_with_different_build_metadata_are_not_equal() {
        let v = |s: &str| Version::try_from(s).unwrap();

        assert_eq!(v("1.0.0+a"), v("1.0.0+a"));
        assert_ne!(v("1.0.0+a"), v("1.0.0+b"));
        assert_ne!(v("1.0.0"), v("1.0.0+a"));
        assert_ne!(v("1.0.0"), v("1.0.0-rc.1"));
    }

    #[test]
    fn equal_versions_have_equal_hashes() {
        use std::collections::HashSet;

        let v = |s: &str| Version::try_from(s).unwrap();
        let set: HashSet<Version> = [v("1.0.0"), v("1.0.0"), v("1.0.0+a"), v("1.0.0-rc.1")]
            .into_iter()
            .collect();

        assert_eq!(set.len(), 3);
        assert!(set.contains(&v("1.0.0+a")));
        assert!(!set.contains(&v("1.0.0+b")));
    }

    #[test]
    fn version_keeps_its_string_form() {
        for s in [
            "0.0.0",
            "1.2.3",
            "1.0.0-alpha.1",
            "1.0.0+build",
            "1.0.0-rc.1+build.5",
            "2.43.3-alpha34",
            "1.0.0+001",
        ] {
            let version = Version::try_from(s).unwrap();
            assert_eq!(version.to_string(), s);
            let json = serde_json::to_string(&version).unwrap();
            assert_eq!(json, format!("\"{s}\""));
            assert_eq!(serde_json::from_str::<Version>(&json).unwrap(), version);
            assert_eq!(Value::from(&version), Value::String(Some(s.to_string())));
        }
    }

    #[test]
    fn deserializing_an_invalid_version_fails() {
        assert!(serde_json::from_str::<Version>("\"1.0\"").is_err());
        assert!(serde_json::from_str::<Version>("\"invalid\"").is_err());
    }
}
