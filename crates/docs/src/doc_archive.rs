use appstate::AppStateData;
use axum::body::{Body, Bytes};
use axum::extract::FromRequest;
use axum::http::Request;
use error::error::{ApiError, ApiResult};
use std::io::Cursor;
use std::path::Path;
use zip::ZipArchive;

type Zip = ZipArchive<Cursor<Vec<u8>>>;

pub struct DocArchive(Zip);

impl DocArchive {
    pub fn extract(&mut self, path: &Path) -> ApiResult<()> {
        match self.0.extract(path) {
            Ok(_) => Ok(()),
            Err(e) => Err(ApiError::from(e)),
        }
    }
}

#[axum::async_trait]
impl FromRequest<AppStateData, Body> for DocArchive {
    type Rejection = ApiError;

    async fn from_request(
        req: Request<Body>,
        state: &AppStateData,
    ) -> Result<Self, Self::Rejection> {
        let data_bytes: Vec<u8> = match Bytes::from_request(req, state).await {
            Ok(b) => b.to_vec(),
            Err(e) => return Err(ApiError::from(&e.to_string())),
        };

        let max_docs_size = state.settings.docs.max_size * 1_000_000;
        if data_bytes.len() > max_docs_size {
            return Err(ApiError::from(&format!(
                "Invalid max. length. {}/{} bytes.",
                data_bytes.len(),
                max_docs_size
            )));
        }

        let reader = Cursor::new(data_bytes);

        match ZipArchive::new(reader) {
            Ok(zip) => Ok(DocArchive(zip)),
            Err(e) => Err(ApiError::from(e)),
        }
    }
}
