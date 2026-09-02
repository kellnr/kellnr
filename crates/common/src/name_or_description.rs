use thiserror::Error;
use utoipa::ToSchema;

#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq, Clone, Hash, ToSchema)]
#[schema(value_type = String)]
pub struct NameOrDescription(pub String);

#[derive(Debug, PartialEq, Eq, Error)]
pub enum NameOrDescriptionError {
    #[error("Invalid length")]
    InvalidLength,
}

impl TryFrom<String> for NameOrDescription {
    type Error = NameOrDescriptionError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.len() > 64 {
            Err(NameOrDescriptionError::InvalidLength)
        } else {
            Ok(Self(value))
        }
    }
}

impl From<NameOrDescription> for String {
    fn from(value: NameOrDescription) -> Self {
        value.0
    }
}
