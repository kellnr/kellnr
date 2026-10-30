use std::fs::DirBuilder;

use object_store::local::LocalFileSystem;

use crate::object_storage::ObjectStorage;
use crate::storage_error::StorageError;

pub type FSStorage = ObjectStorage<LocalFileSystem>;

impl FSStorage {
    pub fn new(crate_folder: &str) -> Result<Self, StorageError> {
        let path = std::path::Path::new(crate_folder);
        if !path.exists() {
            DirBuilder::new()
                .recursive(true)
                .create(crate_folder)
                .map_err(|e| StorageError::CreateBinPath(path.to_path_buf(), e))?;
        }
        let client = LocalFileSystem::new_with_prefix(path)?;
        Ok(Self(client))
    }
}
