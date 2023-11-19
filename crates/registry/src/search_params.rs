use axum::{extract::Query, http::request::Parts, RequestPartsExt};
use common::original_name::OriginalName;
use hyper::StatusCode;
use std::{collections::HashMap, convert::TryFrom, usize};

pub struct SearchParams {
    pub q: OriginalName,
    pub per_page: PerPage,
}

pub struct PerPage(pub usize);

impl TryFrom<usize> for PerPage {
    type Error = &'static str;

    fn try_from(limit: usize) -> Result<Self, Self::Error> {
        if !(0..=100).contains(&limit) {
            Err("per_page limit has to be between 0 and 100.")
        } else {
            Ok(Self(limit))
        }
    }
}

impl From<PerPage> for usize {
    fn from(pp: PerPage) -> Self {
        pp.0
    }
}

#[axum::async_trait]
impl<S> axum::extract::FromRequestParts<S> for SearchParams {
    type Rejection = (StatusCode, String);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let query_params = parts
            .extract::<Query<HashMap<String, String>>>()
            .await
            .map(|Query(params)| params)
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to extract query parameters: {}", e),
                )
            })?;

        let q = query_params
            .get("q")
            .ok_or((StatusCode::BAD_REQUEST, "missing q".to_owned()))?;
        let q = OriginalName::try_from(q).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

        let per_page = query_params
            .get("per_page")
            .unwrap_or(&"10".to_string())
            .parse::<usize>()
            .map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    format!("Invalid value for per_page: {}", e),
                )
            })?;
        let per_page =
            PerPage::try_from(per_page).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

        Ok(Self { q, per_page })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
