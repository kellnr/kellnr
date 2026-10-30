use std::fmt;

use sea_orm::Value;

use crate::original_name::{NameError, OriginalName};

/// The identity of a crate: its name in lowercase with `-` replaced by `_`.
///
/// Cargo and crates.io treat names that only differ in case or in `-` versus
/// `_` as the same crate, so crates are stored, looked up and kept unique by
/// this name. Use [`OriginalName`] for everything that shows or stores the
/// name as published.
#[derive(
    serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Hash,
)]
#[serde(try_from = "String", into = "String")]
pub struct CanonicalName(String);

impl CanonicalName {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl From<&OriginalName> for CanonicalName {
    fn from(name: &OriginalName) -> Self {
        CanonicalName(name.to_lowercase().replace('-', "_"))
    }
}

impl From<OriginalName> for CanonicalName {
    fn from(name: OriginalName) -> Self {
        CanonicalName::from(&name)
    }
}

/// Validates the name with the rules of crate names, in any spelling.
impl TryFrom<&str> for CanonicalName {
    type Error = NameError;

    fn try_from(name: &str) -> Result<Self, Self::Error> {
        OriginalName::try_from(name).map(CanonicalName::from)
    }
}

impl TryFrom<String> for CanonicalName {
    type Error = NameError;

    fn try_from(name: String) -> Result<Self, Self::Error> {
        OriginalName::try_from(name).map(CanonicalName::from)
    }
}

impl From<CanonicalName> for String {
    fn from(name: CanonicalName) -> Self {
        name.0
    }
}

impl From<CanonicalName> for Value {
    fn from(value: CanonicalName) -> Self {
        Value::String(Some(value.0))
    }
}

impl From<&CanonicalName> for Value {
    fn from(value: &CanonicalName) -> Self {
        Value::String(Some(value.0.clone()))
    }
}

impl fmt::Display for CanonicalName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canonical(name: &str) -> String {
        CanonicalName::try_from(name).unwrap().into_inner()
    }

    #[test]
    fn folds_case_and_separators() {
        assert_eq!("foo_bar", canonical("foo-bar"));
        assert_eq!("foo_bar", canonical("Foo_Bar"));
        assert_eq!("foo_bar_baz", canonical("FOO-bar_BAZ"));
        assert_eq!("serde", canonical("serde"));
    }

    #[test]
    fn spellings_of_the_same_crate_are_equal() {
        assert_eq!(
            CanonicalName::try_from("foo-bar").unwrap(),
            CanonicalName::from(&OriginalName::try_from("Foo_Bar").unwrap())
        );
    }

    #[test]
    fn rejects_invalid_names() {
        assert_eq!(
            Err(NameError::InvalidCharacter),
            CanonicalName::try_from("../etc")
        );
        assert_eq!(
            Err(NameError::InvalidCharacter),
            CanonicalName::try_from("_foo")
        );
        assert_eq!(
            Err(NameError::InvalidLength),
            CanonicalName::try_from("a".repeat(65).as_str())
        );
    }

    #[test]
    fn deserialize_validates_and_canonicalizes() {
        let name: CanonicalName = serde_json::from_str(r#""Foo-Bar""#).unwrap();
        assert_eq!("foo_bar", name.as_str());
        assert!(serde_json::from_str::<CanonicalName>(r#""a/b""#).is_err());
    }
}
