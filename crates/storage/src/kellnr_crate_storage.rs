use crate::cached_crate_storage::CachedCrateStorage;
use settings::Settings;
use std::ops::{Deref, DerefMut};

pub struct KellnrCrateStorage(CachedCrateStorage);

impl KellnrCrateStorage {
    pub async fn new(settings: &Settings) -> Result<Self, anyhow::Error> {
        Ok(Self(
            CachedCrateStorage::new(settings.bin_path(), settings).await?,
        ))
    }

    pub async fn delete(&self, crate_name: &str, crate_version: &str) -> Result<(), anyhow::Error> {
        let path = self.0.crate_path(crate_name, crate_version);
        tokio::fs::remove_file(path).await?;
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
