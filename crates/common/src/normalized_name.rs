use crate::original_name::OriginalName;
use std::fmt;

/// Index name is a lowercase version of the crate name

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct NormalizedName(String);

impl NormalizedName {
    pub fn from_unchecked(name: String) -> Self {
        NormalizedName(name)
    }

    pub fn from_unchecked_str(name: &str) -> Self {
        NormalizedName(name.to_owned())
    }
}

impl From<OriginalName> for NormalizedName {
    fn from(name: OriginalName) -> Self {
        NormalizedName(name.to_lowercase())
    }
}

impl From<&OriginalName> for NormalizedName {
    fn from(name: &OriginalName) -> Self {
        NormalizedName(name.to_lowercase())
    }
}

impl fmt::Display for NormalizedName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", &self.0)
    }
}

impl std::ops::Deref for NormalizedName {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
