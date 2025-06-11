use crate::{
    cached_crate_storage::{CachedCrateStorage, DynStorage},
    storage_error::StorageError,
};
use common::util::generate_rand_string;
use settings::Settings;
use std::{
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
};
use tokio::fs::DirBuilder;

pub struct KellnrCrateStorage(CachedCrateStorage);

impl KellnrCrateStorage {
    pub fn new(settings: &Settings, storage: DynStorage) -> Self {
        Self(CachedCrateStorage::new(settings, storage))
    }

    pub async fn create_rand_doc_queue_path(&self) -> Result<PathBuf, StorageError> {
        let rand = generate_rand_string(10);
        let dir = self.doc_queue_path.join(rand);
        Self::create_recursive_path(&dir).await?;

        Ok(dir)
    }

    async fn create_recursive_path(path: &Path) -> Result<(), StorageError> {
        if !path.exists() {
            DirBuilder::new()
                .recursive(true)
                .create(path)
                .await
                .map_err(|e| StorageError::CreateDocQueuePath(path.to_path_buf(), e))?;
        }
        Ok(())
    }
}

impl Deref for KellnrCrateStorage {
    type Target = CachedCrateStorage;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for KellnrCrateStorage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
