use crate::cached_crate_storage::CachedCrateStorage;
use settings::Settings;
use std::ops::{Deref, DerefMut};

pub struct CratesIoCrateStorage(CachedCrateStorage);

impl CratesIoCrateStorage {
    pub async fn new(settings: &Settings) -> Result<Self, anyhow::Error> {
        Ok(Self(
            CachedCrateStorage::new(settings.crates_io_bin_path(), settings).await?,
        ))
    }
}

impl Deref for CratesIoCrateStorage {
    type Target = CachedCrateStorage;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for CratesIoCrateStorage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
