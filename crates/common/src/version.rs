use std::cmp::Ordering;
use std::convert::TryFrom;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::ops::Deref;

#[derive(Debug, Eq, Clone, serde::Serialize, serde::Deserialize)]
pub struct Version(String);

#[derive(Debug, Eq, PartialEq)]
pub enum VersionError {
    InvalidSemVer,
}

impl Version {
    pub fn from_unchecked_str(version: &str) -> Self {
        Self(version.to_string())
    }
}

impl Display for VersionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Invalid SemVer")
    }
}

impl TryFrom<&str> for Version {
    type Error = VersionError;

    fn try_from(version: &str) -> Result<Self, Self::Error> {
        Version::try_from(&version.to_string())
    }
}

impl TryFrom<&String> for Version {
    type Error = VersionError;

    fn try_from(version: &String) -> Result<Self, Self::Error> {
        match semver::Version::parse(version) {
            Ok(sv) => Ok(Version(sv.to_string())),
            Err(_) => Err(VersionError::InvalidSemVer),
        }
    }
}

impl Deref for Version {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Default for Version {
    fn default() -> Self {
        Version(semver::Version::new(0, 0, 0).to_string())
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        let sv1 = semver::Version::parse(&self.to_string()).unwrap();
        let sv2 = semver::Version::parse(&other.to_string()).unwrap();
        sv1.cmp(&sv2)
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Version {
    fn eq(&self, other: &Self) -> bool {
        let sv1 = semver::Version::parse(&self.to_string()).unwrap();
        let sv2 = semver::Version::parse(&other.to_string()).unwrap();
        sv1 == sv2
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", &self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
