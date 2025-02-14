use crate::{
    providers::{
        self, CrateId, CrateObject, S3Storage, StorageProvider,
    },
    storage_error::StorageError,
};
use common::original_name::OriginalName;
use common::util::generate_rand_string;
use common::version::Version;
use moka::future::Cache;
use settings::Settings;
use std::path::{Path, PathBuf};
use tokio::{
    fs::{create_dir_all, DirBuilder, File},
    io::{AsyncReadExt, AsyncWriteExt},
};

pub type CrateCache = Cache<PathBuf, Vec<u8>>;

pub struct CachedCrateStorage {
    crate_folder: PathBuf,
    pub doc_queue_path: PathBuf,
    cache: Option<CrateCache>,
    remote_storage: S3Storage,
}

impl CachedCrateStorage {
    pub async fn new(crate_folder: PathBuf, settings: &Settings) -> Result<Self, StorageError> {
        let settings::s3::S3 {
            enabled,
            access_key,
            secret_key,
            bucket,
            region,
            endpoint,
            allow_http: is_http,
        } = settings.s3.clone();

        let storage = S3Storage::new(
            enabled, region, endpoint, bucket, access_key, secret_key, is_http,
        )
        .map_err(|e| StorageError::S3StorageInitError(e.to_string()))?;

        let cs = Self {
            crate_folder,
            doc_queue_path: settings.doc_queue_path(),
            cache: if settings.registry.cache_size > 0 {
                Some(Cache::new(settings.registry.cache_size))
            } else {
                None
            },
            remote_storage: storage,
        };
        Self::create_bin_path(&cs.crate_folder).await?;
        Ok(cs)
    }

    pub async fn add_bin_package(
        &self,
        name: &OriginalName,
        version: &Version,
        crate_data: &[u8],
    ) -> Result<String, StorageError> {
        if !self.crate_folder.exists() {
            create_dir_all(&self.crate_folder)
                .await
                .map_err(StorageError::CreateCrateFolder)?;
        }

        let file_path = self.crate_path(name, version);
        if Path::new(&file_path).exists() {
            return Err(StorageError::CrateExists(
                name.to_string(),
                version.to_string(),
            ));
        }

        let mut file = File::create(&file_path)
            .await
            .map_err(|e| StorageError::CreateFile(e, file_path.clone()))?;

        file.write_all(crate_data)
            .await
            .map_err(|e| StorageError::WriteCrateFile(file_path.clone(), e))?;

        // Need to flush after write_all. See https://github.com/kellnr/kellnr/issues/311#issuecomment-2138296102
        file.flush()
            .await
            .map_err(|e| StorageError::FlushCrateFile(file_path.clone(), e))?;

        let data = Vec::from(crate_data);
        let kek = bytes::Bytes::from(data).into();
        if self.remote_storage.enabled {
            self.remote_storage
                .put(
                    Some(kek),
                    CrateId::new(format!("{}-{}.crate", name, version)),
                )
                .await
                .map_err(|e| StorageError::S3GenericError(e.to_string()))?;
        }
        Ok(sha256::digest(crate_data))
    }

    pub fn crate_path(&self, name: &str, version: &str) -> PathBuf {
        self.crate_folder
            .join(format!("{}-{}.crate", name, version))
    }

    async fn create_bin_path(crate_path: &Path) -> Result<(), StorageError> {
        if !crate_path.exists() {
            DirBuilder::new()
                .recursive(true)
                .create(&crate_path)
                .await
                .map_err(|e| StorageError::CreateBinPath(crate_path.to_path_buf(), e))?;
        }
        Ok(())
    }

    pub async fn get_file(&self, file_path: PathBuf) -> Option<Vec<u8>> {
        async fn read_from_fs_or_s3(
            file_path: &PathBuf,
            storage: &impl providers::StorageProvider<Object = CrateObject, ObjectId = CrateId>,
        ) -> Option<Vec<u8>> {
            let mut file = File::open(&file_path).await.ok()?;
            let mut krate = Vec::new();
            file.read_to_end(&mut krate).await.ok()?;

            if krate.is_empty() {
                if let Some(path) = &file_path.to_str() {
                    let got = read_from_s3(storage, path).await.ok()?;
                    return got;
                }
            }

            Some(krate)
        }

        async fn read_from_s3(
            storage: &impl providers::StorageProvider<Object = CrateObject, ObjectId = CrateId>,
            path: &str,
        ) -> Result<Option<Vec<u8>>, StorageError> {
            let search_param = CrateId::new(path.to_string());

            let res = storage
                .get(search_param)
                .await
                .map_err(|e| StorageError::S3GenericError(e.to_string()))?;

            if res.0.is_empty() {
                Ok(None)
            } else {
                Ok(Some(res.0.into()))
            }
        }

        async fn from_cache(
            cache: &CrateCache,
            file_path: PathBuf,
            storage: &impl providers::StorageProvider<Object = CrateObject, ObjectId = CrateId>,
        ) -> Option<Vec<u8>> {
            match cache.get(&file_path).await {
                None => {
                    let krate = read_from_fs_or_s3(&file_path, storage).await?;
                    cache.insert(file_path, krate.clone()).await;
                    Some(krate)
                }
                Some(krate) => Some(krate.to_owned()),
            }
        }

        match &self.cache {
            None => {
                let krate = read_from_fs_or_s3(&file_path, &self.remote_storage).await?;
                Some(krate)
            }
            Some(c) => from_cache(c, file_path, &self.remote_storage).await,
        }
    }

    pub async fn invalidate_path(&self, file_path: &PathBuf) {
        if let Some(cache) = &self.cache {
            cache.invalidate(file_path).await;
        }
    }

    pub fn cache_has_path(&self, file_path: &PathBuf) -> bool {
        if let Some(cache) = &self.cache {
            cache.contains_key(file_path)
        } else {
            false
        }
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
