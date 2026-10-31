use axum::extract::{FromRequestParts, Query};
use axum::http::request::Parts;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use crate::error::RouteError;

/// Default number of crates per page.
const DEFAULT_PAGE_SIZE: u64 = 10;
/// Upper bound of crates per page.
const MAX_PAGE_SIZE: u64 = 100;

/// Number of entries per page, within `1..=MAX_PAGE_SIZE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageSize(u64);

impl PageSize {
    /// Clamps the requested size into the allowed range.
    pub fn new(requested: Option<u64>) -> Self {
        Self(
            requested
                .unwrap_or(DEFAULT_PAGE_SIZE)
                .clamp(1, MAX_PAGE_SIZE),
        )
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

/// Query parameters of a paginated list.
#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct PageParams {
    /// Number of the page, starting at 0
    page: Option<u64>,
    /// Entries per page, 10 by default, at most 100
    page_size: Option<u64>,
}

/// A page of a list, read from the `page` and `page_size` query parameters.
/// The offset of the page always fits the database.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageRequest {
    page: u64,
    size: PageSize,
    offset: u64,
}

impl PageRequest {
    /// Rejects a page whose offset does not fit the database with 400.
    pub fn new(page: Option<u64>, page_size: Option<u64>) -> Result<Self, RouteError> {
        let size = PageSize::new(page_size);
        let page = page.unwrap_or(0);
        let offset = page
            .checked_mul(size.get())
            .filter(|o| i64::try_from(*o).is_ok())
            .ok_or_else(|| RouteError::BadRequest(format!("Page {page} is out of range")))?;
        Ok(Self { page, size, offset })
    }

    pub fn page(&self) -> u64 {
        self.page
    }

    pub fn size(&self) -> PageSize {
        self.size
    }

    pub fn offset(&self) -> u64 {
        self.offset
    }
}

impl<S: Send + Sync> FromRequestParts<S> for PageRequest {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Query(params) = Query::<PageParams>::from_request_parts(parts, state)
            .await
            .map_err(IntoResponse::into_response)?;
        Self::new(params.page, params.page_size).map_err(IntoResponse::into_response)
    }
}

#[cfg(test)]
mod tests {
    use axum::http::{Request, StatusCode};

    use super::*;

    async fn extract(query: &str) -> Result<PageRequest, StatusCode> {
        let (mut parts, ()) = Request::get(format!("/crates?{query}"))
            .body(())
            .unwrap()
            .into_parts();
        PageRequest::from_request_parts(&mut parts, &())
            .await
            .map_err(|r| r.status())
    }

    fn page(page: u64, size: u64, offset: u64) -> PageRequest {
        PageRequest {
            page,
            size: PageSize(size),
            offset,
        }
    }

    #[tokio::test]
    async fn defaults_to_the_first_page_of_ten() {
        assert_eq!(extract("").await, Ok(page(0, 10, 0)));
        assert_eq!(extract("sort=downloads").await, Ok(page(0, 10, 0)));
    }

    #[tokio::test]
    async fn page_size_is_clamped() {
        assert_eq!(extract("page_size=0&page=2").await, Ok(page(2, 1, 2)));
        assert_eq!(
            extract("page_size=1000&page=2").await,
            Ok(page(2, 100, 200))
        );
        assert_eq!(extract("page_size=20&page=3").await, Ok(page(3, 20, 60)));
    }

    #[tokio::test]
    async fn page_out_of_range_is_a_bad_request() {
        assert_eq!(
            extract(&format!("page={}", u64::MAX)).await,
            Err(StatusCode::BAD_REQUEST)
        );
        let max = i64::MAX.unsigned_abs();
        assert_eq!(
            extract(&format!("page={}&page_size=1", max + 1)).await,
            Err(StatusCode::BAD_REQUEST)
        );
        assert_eq!(
            extract(&format!("page={max}&page_size=1")).await,
            Ok(page(max, 1, max))
        );
    }

    #[tokio::test]
    async fn invalid_numbers_are_a_bad_request() {
        assert_eq!(extract("page=abc").await, Err(StatusCode::BAD_REQUEST));
        assert_eq!(extract("page_size=-1").await, Err(StatusCode::BAD_REQUEST));
    }
}
