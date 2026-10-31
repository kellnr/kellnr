use std::io::Cursor;
use std::path::Path;

use axum::body::{Body, Bytes};
use axum::extract::FromRequest;
use axum::http::Request;
use kellnr_http::error::{ApiError, ApiResult};
use kellnr_http::state::AppStateData;
use tracing::warn;
use zip::ZipArchive;
use zip::result::ZipError;

use crate::docs_error::DocsError;
use crate::unpack::ExtractLimits;

type Zip = ZipArchive<Cursor<Vec<u8>>>;

pub struct DocArchive {
    zip: Zip,
    limits: ExtractLimits,
}

impl DocArchive {
    /// Extract all regular files and directories into `path`.
    ///
    /// Symlinks are skipped, so the extracted tree only contains data from the
    /// archive itself. Entries that would escape `path` or exceed the
    /// extraction limits reject the archive.
    pub fn extract(&mut self, path: &Path) -> ApiResult<()> {
        let mut budget = self.limits.budget();
        for index in 0..self.zip.len() {
            budget.take_entry()?;
            let mut file = self.zip.by_index(index)?;
            let relative = file
                .enclosed_name()
                .ok_or(ZipError::InvalidArchive("Invalid file path".into()))?;
            let target = path.join(relative);
            if file.is_symlink() {
                warn!("Skipping symlink in docs archive: {}", target.display());
            } else if file.is_dir() {
                std::fs::create_dir_all(&target)?;
            } else {
                budget.write_file(&mut file, &target)?;
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
            .map_err(DocsError::ExtractBytesFailed)?
            .to_vec();

        let max_docs_size = state.settings.docs.max_bytes();
        if data_bytes.len() > max_docs_size {
            return Err(DocsError::TooLarge(data_bytes.len(), max_docs_size).into());
        }

        let reader = Cursor::new(data_bytes);

        match ZipArchive::new(reader) {
            Ok(zip) => Ok(DocArchive {
                zip,
                limits: ExtractLimits::from_max_docs_size(state.settings.docs.max_size),
            }),
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

    fn archive_with_limits(
        limits: ExtractLimits,
        build: impl FnOnce(&mut ZipWriter<Cursor<Vec<u8>>>),
    ) -> DocArchive {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        build(&mut zip);
        let bytes = zip.finish().unwrap().into_inner();
        DocArchive {
            zip: ZipArchive::new(Cursor::new(bytes)).unwrap(),
            limits,
        }
    }

    fn archive(build: impl FnOnce(&mut ZipWriter<Cursor<Vec<u8>>>)) -> DocArchive {
        archive_with_limits(ExtractLimits::from_max_docs_size(1), build)
    }

    fn zip_of_zeros(files: usize, size: usize) -> impl FnOnce(&mut ZipWriter<Cursor<Vec<u8>>>) {
        move |zip| {
            for i in 0..files {
                zip.start_file(format!("doc/{i}.html"), SimpleFileOptions::default())
                    .unwrap();
                zip.write_all(&vec![0; size]).unwrap();
            }
        }
    }

    #[tokio::test]
    async fn rejects_archive_larger_than_max_size_with_413() {
        let mut settings = kellnr_settings::test_settings();
        settings.docs.max_size = 0;
        let state = AppStateData {
            settings: std::sync::Arc::new(settings),
            ..kellnr_http::test_support::test_state()
        };
        let request = Request::builder().body(Body::from("zip")).unwrap();

        let err = DocArchive::from_request(request, &state)
            .await
            .err()
            .unwrap();

        assert_eq!(
            axum::response::IntoResponse::into_response(err).status(),
            axum::http::StatusCode::PAYLOAD_TOO_LARGE
        );
    }

    #[test]
    fn extract_rejects_archive_exceeding_uncompressed_size() {
        // Zeros compress well, so the archive itself stays tiny.
        let mut docs = archive_with_limits(ExtractLimits::new(10_000, 10), zip_of_zeros(2, 6_000));
        let dest = tempfile::tempdir().unwrap();

        let err = docs.extract(dest.path()).err().unwrap();

        assert_eq!(
            axum::response::IntoResponse::into_response(err).status(),
            axum::http::StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn extract_rejects_archive_with_too_many_entries() {
        let mut docs = archive_with_limits(ExtractLimits::new(10_000, 10), zip_of_zeros(11, 0));
        let dest = tempfile::tempdir().unwrap();

        assert!(docs.extract(dest.path()).is_err());
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
