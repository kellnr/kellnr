use crate::normalized_name::NormalizedName;
use regex::Regex;
use rocket::data::ToByteUnit;
use rocket::form::{self, FromFormField};
use rocket::http::RawStr;
use rocket::request::FromParam;
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
    pub fn unchecked(name: String) -> Self {
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

impl TryFrom<&RawStr> for OriginalName {
    type Error = NameError;

    fn try_from(package_name: &RawStr) -> Result<Self, Self::Error> {
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

impl<'r> FromParam<'r> for OriginalName {
    type Error = &'r str;

    fn from_param(param: &'r str) -> Result<Self, Self::Error> {
        let pkg_name = OriginalName::try_from(param);

        match pkg_name {
            Ok(name) => Ok(name),
            Err(NameError::InvalidCharacter) =>
                Err("Invalid crate name: Only alphanumerical characters and \"-\" \"_\" are allowed. First character must be alphabetic."),
            Err(NameError::InvalidLength) =>
                Err("Invalid crate name: Max allowed length is 64 characters.")
        }
    }
}

#[rocket::async_trait]
impl<'r> FromFormField<'r> for OriginalName {
    fn from_value(field: rocket::form::ValueField<'r>) -> rocket::form::Result<'r, Self> {
        let pkg_name = OriginalName::try_from(field.value);
        match pkg_name {
            Ok(name) => Ok(name),
            Err(NameError::InvalidCharacter) =>
                return Err(form::Error::validation("Invalid crate name: Only alphanumerical characters and \"-\" \"_\" are allowed. First character must be alphabetic.").into()),
            Err(NameError::InvalidLength) =>
                return Err(form::Error::validation("Invalid crate name: Max allowed length is 64 characters.").into())
        }
    }

    async fn from_data(field: rocket::form::DataField<'r, '_>) -> rocket::form::Result<'r, Self> {
        // Retrieve the configured data limit or use `1KiB` as default.
        let limit = field
            .request
            .limits()
            .get("name")
            .unwrap_or_else(|| 1_u64.kibibytes());

        // Read the capped data stream, returning a limit error as needed.
        let bytes = field.data.open(limit).into_bytes().await?;
        if !bytes.is_complete() {
            return Err((None, Some(limit)).into());
        }

        // Store the bytes in request-local cache and split at ':'.
        let bytes = bytes.into_inner();
        let bytes = rocket::request::local_cache!(field.request, bytes);

        // Try to parse the name as UTF-8 or return an error if it fails.
        let name = std::str::from_utf8(bytes)?;
        let pkg_name = OriginalName::try_from(name);
        match pkg_name {
            Ok(name) => Ok(name),
            Err(NameError::InvalidCharacter) =>
                return Err(form::Error::validation("Invalid crate name: Only alphanumerical characters and \"-\" \"_\" are allowed. First character must be alphabetic.").into()),
            Err(NameError::InvalidLength) =>
                return Err(form::Error::validation("Invalid crate name: Max allowed length is 64 characters.").into())
        }
    }

    fn default() -> Option<Self> {
        None
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
