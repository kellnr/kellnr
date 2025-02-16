use crate::{cached_crate_storage::CachedCrateStorage, storage_error::StorageError};
use settings::Settings;
use std::ops::{Deref, DerefMut};

pub struct KellnrCrateStorage(CachedCrateStorage);

impl KellnrCrateStorage {
    pub async fn new(settings: &Settings) -> Result<Self, StorageError> {
        Ok(Self(
            CachedCrateStorage::new(settings.bin_path(), settings, settings.s3.enabled.into())
                .await?,
        ))
    }

    pub async fn delete(&self, crate_name: &str, crate_version: &str) -> Result<(), StorageError> {
        let path = self.0.crate_path(crate_name, crate_version);
        tokio::fs::remove_file(&path)
            .await
            .map_err(|e| StorageError::RemoveFile(path.clone(), e))?;
        self.0.invalidate_path(&path).await;
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
