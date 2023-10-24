use rocket::data::ToByteUnit;
use rocket::form::{self, DataField, FromFormField, ValueField};
use serde::Deserialize;
use std::convert::TryFrom;

#[derive(Deserialize)]
pub struct PerPage(pub i32);

impl TryFrom<i32> for PerPage {
    type Error = &'static str;

    fn try_from(limit: i32) -> Result<Self, Self::Error> {
        if !(0..=100).contains(&limit) {
            Err("per_page limit has to be between 0 and 100.")
        } else {
            Ok(Self(limit))
        }
    }
}

impl From<PerPage> for i32 {
    fn from(pp: PerPage) -> Self {
        pp.0
    }
}

#[rocket::async_trait]
impl<'r> FromFormField<'r> for PerPage {
    fn from_value(field: ValueField<'r>) -> form::Result<'r, Self> {
        match field.value.parse::<i32>() {
            Ok(limit) => match Self::try_from(limit) {
                Ok(pp) => Ok(pp),
                Err(e) => return Err(form::Error::validation(e).into()),
            },
            _ => {
                return Err(form::Error::validation("per_page has to be an integer value.").into())
            }
        }
    }

    async fn from_data(field: DataField<'r, '_>) -> form::Result<'r, Self> {
        // Retrieve the configured data limit or use `8bytes` as default.
        let limit = field
            .request
            .limits()
            .get("per_page")
            .unwrap_or_else(|| 8.bytes());

        // Read the capped data stream, returning a limit error as needed.
        let value = field.data.open(limit).into_string().await?;
        if !value.is_complete() {
            return Err((None, Some(limit)).into());
        }

        // Store the bytes in request-local cache and split at ':'.
        let value = value.into_inner();
        let value = rocket::request::local_cache!(field.request, value);

        match value.parse::<i32>() {
            Ok(limit) => match Self::try_from(limit) {
                Ok(pp) => Ok(pp),
                Err(e) => return Err(form::Error::validation(e).into()),
            },
            _ => {
                return Err(form::Error::validation("per_page has to be an integer value.").into())
            }
        }
    }

    fn default() -> Option<Self> {
        Some(PerPage(10))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_from_too_small() {
        let result = PerPage::try_from(-1);
        assert!(result.is_err());
    }

    #[test]
    fn try_from_too_large() {
        let result = PerPage::try_from(101);
        assert!(result.is_err());
    }

    #[test]
    fn try_from_valid() {
        let result = PerPage::try_from(20);
        assert!(result.is_ok());
    }

    #[test]
    fn into_i32() {
        let result = PerPage::try_from(20).unwrap();
        let as_int: i32 = result.into();
        assert_eq!(20, as_int);
    }
}
