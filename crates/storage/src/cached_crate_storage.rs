use crate::{
    storage_error::StorageError,
    storage_provider::{self, FSStorage, S3Storage},
};
use common::original_name::OriginalName;
use common::util::generate_rand_string;
use common::version::Version;
use moka::future::Cache;
use settings::{Settings, s3::S3};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::fs::DirBuilder;

pub type CrateCache = Cache<PathBuf, Vec<u8>>;

pub struct CachedCrateStorage {
    crate_folder: String,
    pub doc_queue_path: PathBuf,
    storage: storage_provider::Storage,
}

impl CachedCrateStorage {
    pub async fn new(crate_folder: &str, settings: &Settings) -> Result<Self, StorageError> {
        let storage: storage_provider::Storage = if settings.s3.enabled {
            let S3 {
                enabled: _,
                access_key,
                secret_key,
                region,
                endpoint,
                allow_http,
            } = &settings.s3;
            let mut bucket: Vec<&str> = crate_folder.split("/").collect();
            bucket.reverse();
            let bucket = bucket
                .first()
                .ok_or(StorageError::StorageInitError(format!(
                    "Wrong bucket name: {}",
                    crate_folder
                )))?;

            S3Storage::new(
                region,
                endpoint,
                bucket,
                access_key,
                secret_key,
                *allow_http,
            )
            .map_err(|e| StorageError::StorageInitError(e.to_string()))?
            .into()
        } else {
            let path = std::path::Path::new(crate_folder);
            if !path.exists() {
                DirBuilder::new()
                    .recursive(true)
                    .create(&crate_folder)
                    .await
                    .map_err(|e| StorageError::CreateBinPath(path.to_path_buf(), e))?;
            }
            FSStorage::new(crate_folder, settings.registry.cache_size)
                .map_err(|e| StorageError::StorageInitError(e.to_string()))?
                .into()
        };

        let cs = Self {
            crate_folder: crate_folder.to_owned(),
            doc_queue_path: settings.doc_queue_path(),
            storage,
        };
        Ok(cs)
    }

    pub async fn remove_bin(
        &self,
        name: &OriginalName,
        version: &Version,
    ) -> Result<(), StorageError> {
        let crate_path = self.crate_path(name, version);
        self.storage.put(&crate_path, None).await.map_err(|e| {
            StorageError::GenericError(format!(
                "Error while removing bin from storage. Error: {}",
                e
            ))
        })?;
        Ok(())
    }

    pub async fn add_bin_package(
        &self,
        name: &OriginalName,
        version: &Version,
        crate_data: Arc<[u8]>,
    ) -> Result<String, StorageError> {
        let crate_path = self.crate_path(name, version);
        self.storage
            .put(&crate_path, Some(crate_data.clone().to_vec().into()))
            .await
            .map_err(|e| {
                if let StorageError::S3Error(object_store::Error::AlreadyExists {
                    path: _,
                    source: _,
                }) = e
                {
                    return StorageError::CrateExists(name.to_string(), version.to_string());
                }
                StorageError::GenericError(format!("Error while adding bin package. Error: {}", e))
            })?;

        Ok(sha256::digest(&*crate_data))
    }

    pub fn crate_path(&self, name: &str, version: &str) -> String {
        format!("{}/{}-{}.crate", &self.crate_folder, name, version)
    }

    pub async fn get_file(&self, file_path: &str) -> Option<Vec<u8>> {
        self.storage.get(file_path).await.map(<Vec<u8>>::from).ok()
    }

    pub async fn create_rand_doc_queue_path(&self) -> Result<PathBuf, StorageError> {
        let rand = generate_rand_string(10);
        let dir = self.doc_queue_path.join(rand);
        Self::create_recursive_path(&dir).await?;

        Ok(dir)
    }

    pub fn cache_has_path(&self, file_path: &PathBuf) -> bool {
        match &self.storage {
            storage_provider::Storage::S3(_) => false,
            storage_provider::Storage::FS(fsstorage) => fsstorage.cache_has_path(file_path),
        }
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
