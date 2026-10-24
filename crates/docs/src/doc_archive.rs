use std::io::Cursor;
use std::path::Path;

use axum::body::{Body, Bytes};
use axum::extract::FromRequest;
use axum::http::Request;
use kellnr_appstate::AppStateData;
use kellnr_error::api_error::{ApiError, ApiResult};
use kellnr_registry::registry_error::RegistryError;
use tracing::warn;
use zip::ZipArchive;
use zip::result::ZipError;

use crate::unpack::write_file;

type Zip = ZipArchive<Cursor<Vec<u8>>>;

pub struct DocArchive(Zip);

impl DocArchive {
    /// Extract all regular files and directories into `path`.
    ///
    /// Symlinks are skipped, so the extracted tree only contains data from the
    /// archive itself. Entries that would escape `path` reject the archive.
    pub fn extract(&mut self, path: &Path) -> ApiResult<()> {
        for index in 0..self.0.len() {
            let mut file = self.0.by_index(index)?;
            let relative = file
                .enclosed_name()
                .ok_or(ZipError::InvalidArchive("Invalid file path".into()))?;
            let target = path.join(relative);
            if file.is_symlink() {
                warn!("Skipping symlink in docs archive: {}", target.display());
            } else if file.is_dir() {
                std::fs::create_dir_all(&target)?;
            } else {
                write_file(&mut file, &target)?;
            }
        }
        Ok(())
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

#[cfg(test)]
mod tests {
    use std::io::Write;

    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    use super::*;

    fn archive(build: impl FnOnce(&mut ZipWriter<Cursor<Vec<u8>>>)) -> DocArchive {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        build(&mut zip);
        let bytes = zip.finish().unwrap().into_inner();
        DocArchive(ZipArchive::new(Cursor::new(bytes)).unwrap())
    }

    #[test]
    fn extract_writes_files_and_skips_symlinks() {
        let secret = tempfile::NamedTempFile::new().unwrap();
        let mut docs = archive(|zip| {
            let options = SimpleFileOptions::default();
            zip.start_file("doc/index.html", options).unwrap();
            zip.write_all(b"docs").unwrap();
            zip.add_symlink("doc/db.html", secret.path().to_str().unwrap(), options)
                .unwrap();
        });
        let dest = tempfile::tempdir().unwrap();

        assert!(docs.extract(dest.path()).is_ok());

        assert_eq!(
            std::fs::read(dest.path().join("doc/index.html")).unwrap(),
            b"docs"
        );
        assert!(std::fs::symlink_metadata(dest.path().join("doc/db.html")).is_err());
    }
}
