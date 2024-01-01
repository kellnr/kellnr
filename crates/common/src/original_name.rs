use crate::normalized_name::NormalizedName;
use regex::Regex;
use std::convert::TryFrom;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::ops::Deref;

#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct OriginalName(String);

#[derive(Debug, PartialEq, Eq)]
pub enum NameError {
    InvalidCharacter,
    InvalidLength,
}

impl OriginalName {
    pub fn to_normalized(&self) -> NormalizedName {
        NormalizedName::from(self)
    }
    pub fn from_unchecked_str(name: String) -> Self {
        Self(name)
    }
}

impl TryFrom<String> for OriginalName {
    type Error = NameError;

    fn try_from(package_name: String) -> Result<Self, Self::Error> {
        let re = Regex::new(r"^[a-zA-Z][a-zA-Z0-9-_]*$").unwrap();

        if !re.is_match(&package_name) {
            Err(NameError::InvalidCharacter)
        } else if package_name.len() > 64 {
            Err(NameError::InvalidLength)
        } else {
            Ok(OriginalName(package_name))
        }
    }
}

impl Display for NameError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            NameError::InvalidCharacter => {
                write!(f, "Invalid character in name")
            }
            NameError::InvalidLength => {
                write!(f, "Invalid length for name")
            }
        }
    }
}

impl From<&OriginalName> for String {
    fn from(name: &OriginalName) -> Self {
        name.to_string()
    }
}

impl From<OriginalName> for String {
    fn from(name: OriginalName) -> Self {
        name.to_string()
    }
}

impl TryFrom<&String> for OriginalName {
    type Error = NameError;

    fn try_from(package_name: &String) -> Result<Self, Self::Error> {
        OriginalName::try_from(package_name.to_string())
    }
}

impl TryFrom<&str> for OriginalName {
    type Error = NameError;

    fn try_from(package_name: &str) -> Result<Self, Self::Error> {
        OriginalName::try_from(package_name.to_string())
    }
}

impl Deref for OriginalName {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl fmt::Display for OriginalName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", &self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_package_names() {
        assert_eq!(
            OriginalName::try_from("test-lib").unwrap(),
            OriginalName("test-lib".to_string())
        );
        assert_eq!(
            OriginalName::try_from("Test_lib").unwrap(),
            OriginalName("Test_lib".to_string())
        );
        assert_eq!(
            OriginalName::try_from("test_lib_foo").unwrap(),
            OriginalName("test_lib_foo".to_string())
        );
        assert_eq!(
            OriginalName::try_from("test-lIB-foo").unwrap(),
            OriginalName("test-lIB-foo".to_string())
        );
        assert_eq!(
            OriginalName::try_from("test12_14f").unwrap(),
            OriginalName("test12_14f".to_string())
        );
        assert_eq!(
            OriginalName::try_from("tEs2-23_1f").unwrap(),
            OriginalName("tEs2-23_1f".to_string())
        );
        assert_eq!(
            OriginalName::try_from("testlib").unwrap(),
            OriginalName("testlib".to_string())
        );
        assert_eq!(
            OriginalName::try_from("Testlib23").unwrap(),
            OriginalName("Testlib23".to_string())
        );
    }

    #[test]
    fn invalid_characters_in_package_name() {
        assert_eq!(
            OriginalName::try_from("_test").unwrap_err(),
            NameError::InvalidCharacter
        );
        assert_eq!(
            OriginalName::try_from("44Test").unwrap_err(),
            NameError::InvalidCharacter
        );
        assert_eq!(
            OriginalName::try_from("-Av").unwrap_err(),
            NameError::InvalidCharacter
        );
        assert_eq!(
            OriginalName::try_from("testÄ").unwrap_err(),
            NameError::InvalidCharacter
        );
        assert_eq!(
            OriginalName::try_from("test?").unwrap_err(),
            NameError::InvalidCharacter
        );
        assert_eq!(
            OriginalName::try_from(".45A").unwrap_err(),
            NameError::InvalidCharacter
        );
        assert_eq!(
            OriginalName::try_from("").unwrap_err(),
            NameError::InvalidCharacter
        );
    }

    #[test]
    fn too_long_package_name() {
        assert_eq!(
            OriginalName::try_from(
                "zbsfrofdgxekytxrporaocoieaviehlvjrroockxufdkzgtxudkmdkentyyhkmtpx"
            )
            .unwrap_err(),
            NameError::InvalidLength
        );
    }
}
