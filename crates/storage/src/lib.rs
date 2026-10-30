pub mod cached_crate_storage;
pub mod cratesio_crate_storage;
pub mod docs_storage;
pub mod fs_storage;
pub mod gcs_storage;
pub mod kellnr_crate_storage;
pub mod object_storage;
pub mod s3_storage;
pub mod storage;
pub mod storage_error;
pub mod toolchain_storage;

use kellnr_settings::Settings;

use crate::cached_crate_storage::DynStorage;
use crate::fs_storage::FSStorage;
use crate::gcs_storage::GCSStorage;
use crate::s3_storage::S3Storage;
use crate::storage_error::StorageError;

/// The configured storage backend for `folder`, which is the bucket name for
/// S3 and GCS.
pub fn init_storage(folder: &str, settings: &Settings) -> Result<DynStorage, StorageError> {
    Ok(if settings.s3.enabled {
        Box::new(S3Storage::try_from((folder, settings))?)
    } else if settings.gcs.enabled {
        Box::new(GCSStorage::try_from((folder, settings))?)
    } else {
        Box::new(FSStorage::new(folder)?)
    })
}
