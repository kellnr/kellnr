use crate::cached_crate_storage::{CachedCrateStorage, DynStorage};
use settings::Settings;
use std::ops::{Deref, DerefMut};

pub struct CratesIoCrateStorage(CachedCrateStorage);

impl CratesIoCrateStorage {
    pub fn new(settings: &Settings, storage: DynStorage) -> Self {
        Self(CachedCrateStorage::new(settings, storage))
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
