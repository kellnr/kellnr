use crate::{cached_crate_storage::CachedCrateStorage, storage_error::StorageError};
use common::{original_name::OriginalName, version::Version};
use settings::Settings;
use std::ops::{Deref, DerefMut};

pub struct KellnrCrateStorage(CachedCrateStorage);

impl KellnrCrateStorage {
    pub async fn new(settings: &Settings) -> Result<Self, StorageError> {
        Ok(Self(
            CachedCrateStorage::new(settings.crates_path().as_str(), settings).await?,
        ))
    }

    pub async fn delete(
        &self,
        crate_name: &OriginalName,
        crate_version: &Version,
    ) -> Result<(), StorageError> {
        let path = self.0.crate_path(crate_name, crate_version);
        self.remove_bin(crate_name, crate_version).await?;
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
