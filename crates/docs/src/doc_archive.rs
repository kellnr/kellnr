use appstate::AppStateData;
use axum::body::{Body, Bytes};
use axum::extract::FromRequest;
use axum::http::Request;
use error::api_error::{ApiError, ApiResult};
use registry::registry_error::RegistryError;
use std::io::Cursor;
use std::path::Path;
use zip::ZipArchive;

type Zip = ZipArchive<Cursor<Vec<u8>>>;

pub struct DocArchive(Zip);

impl DocArchive {
    pub fn extract(&mut self, path: &Path) -> ApiResult<()> {
        Ok(self.0.extract(path)?)
    }
}

impl FromRequest<AppStateData, Body> for DocArchive {
    type Rejection = ApiError;

    async fn from_request(
        req: Request<Body>,
        state: &AppStateData,
    ) -> Result<Self, Self::Rejection> {
        let data_bytes: Vec<u8> = Bytes::from_request(req, state)
            .await
            .map_err(RegistryError::ExtractBytesFailed)?
            .to_vec();

        let max_docs_size = state.settings.docs.max_size * 1_000_000;
        if data_bytes.len() > max_docs_size {
            return Err(RegistryError::InvalidMinLength(data_bytes.len(), max_docs_size).into());
        }

        let reader = Cursor::new(data_bytes);

        match ZipArchive::new(reader) {
            Ok(zip) => Ok(DocArchive(zip)),
            Err(e) => Err(ApiError::from(e)),
        }
    }
}
