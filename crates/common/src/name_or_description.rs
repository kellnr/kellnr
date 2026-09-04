use std::ops::Deref;

use thiserror::Error;
use utoipa::ToSchema;

/// A free-form crate search query, matched against crate names and descriptions.
///
/// Unlike [`crate::original_name::OriginalName`] the content is not restricted
/// to crate-name characters, because a description search takes arbitrary text.
/// Only the length is bounded, so an oversized query cannot turn into an
/// oversized `LIKE` pattern scanned across every row.
#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq, Clone, Hash, ToSchema)]
#[schema(value_type = String)]
// Validate on every deserialization (query strings, JSON bodies) instead of only
// in the explicit `TryFrom` constructor. Otherwise serde-based extractors such as
// the web UI's `Query<SearchParams>` bypass the length bound entirely. Mirrors
// the custom `Deserialize` on `OriginalName`.
#[serde(try_from = "String")]
pub struct NameOrDescription(String);

/// Upper bound on a search query, in characters.
const MAX_LEN: usize = 64;

#[derive(Debug, PartialEq, Eq, Error)]
pub enum NameOrDescriptionError {
    #[error("Search query must not be longer than {MAX_LEN} characters")]
    InvalidLength,
}

impl NameOrDescription {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl TryFrom<String> for NameOrDescription {
    type Error = NameOrDescriptionError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.chars().count() > MAX_LEN {
            Err(NameOrDescriptionError::InvalidLength)
        } else {
            Ok(Self(value))
        }
    }
}

impl TryFrom<&str> for NameOrDescription {
    type Error = NameOrDescriptionError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::try_from(value.to_string())
    }
}

impl From<NameOrDescription> for String {
    fn from(value: NameOrDescription) -> Self {
        value.0
    }
}

impl Deref for NameOrDescription {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_arbitrary_text() {
        assert_eq!(
            NameOrDescription::try_from("vcard parser")
                .unwrap()
                .as_str(),
            "vcard parser"
        );
    }

    #[test]
    fn accepts_query_at_the_limit() {
        let query = "a".repeat(MAX_LEN);
        assert_eq!(
            NameOrDescription::try_from(query.clone()).unwrap().as_str(),
            query
        );
    }

    #[test]
    fn rejects_query_over_the_limit() {
        let query = "a".repeat(MAX_LEN + 1);
        assert_eq!(
            NameOrDescription::try_from(query),
            Err(NameOrDescriptionError::InvalidLength)
        );
    }

    // The bound counts characters, not bytes, so a query of multi-byte
    // characters is not rejected earlier than an ASCII one of the same length.
    #[test]
    fn counts_characters_not_bytes() {
        let query = "ä".repeat(MAX_LEN);
        assert!(query.len() > MAX_LEN);
        assert!(NameOrDescription::try_from(query).is_ok());
    }

    // The length bound must hold for serde-based extractors too, not only for
    // the explicit `TryFrom` constructor.
    #[test]
    fn deserialization_enforces_the_limit() {
        let ok =
            serde_json::from_value::<NameOrDescription>(serde_json::json!("a".repeat(MAX_LEN)));
        assert!(ok.is_ok());

        let too_long =
            serde_json::from_value::<NameOrDescription>(serde_json::json!("a".repeat(MAX_LEN + 1)));
        assert!(too_long.is_err());
    }
}
