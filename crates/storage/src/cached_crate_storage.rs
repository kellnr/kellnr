use anyhow::{bail, Context, Result};
use common::original_name::OriginalName;
use common::util::generate_rand_string;
use common::version::Version;
use hex::ToHex;
use moka::future::Cache;
use settings::Settings;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::{
    fs::{create_dir_all, DirBuilder, File},
    io::{AsyncReadExt, AsyncWriteExt},
};
use tracing::{error, warn};

pub type CrateCache = Cache<PathBuf, Vec<u8>>;

pub struct CachedCrateStorage {
    crate_folder: PathBuf,
    pub doc_queue_path: PathBuf,
    cache: Option<CrateCache>,
}

impl CachedCrateStorage {
    pub async fn new(crate_folder: PathBuf, settings: &Settings) -> Result<Self, anyhow::Error> {
        let cs = Self {
            crate_folder,
            doc_queue_path: settings.doc_queue_path(),
            cache: if settings.registry.cache_size > 0 {
                Some(Cache::new(settings.registry.cache_size))
            } else {
                None
            },
        };
        Self::create_bin_path(&cs.crate_folder).await?;
        Ok(cs)
    }

    pub async fn add_bin_package(
        &self,
        name: &OriginalName,
        version: &Version,
        crate_data: &[u8],
    ) -> Result<String> {
        if !self.crate_folder.exists() {
            create_dir_all(&self.crate_folder)
                .await
                .with_context(|| "Failed to create crate_folder")?
        }

        let file_path = self.crate_path(name, version);
        if Path::new(&file_path).exists() {
            bail!("Crate with version already exists: {}-{}", &name, &version)
        }

        let mut file = File::create(&file_path).await.with_context(|| {
            format!(
                "Unable to create file on storage: {}",
                file_path.to_string_lossy()
            )
        })?;

        file.write_all(crate_data).await.with_context(|| {
            format!(
                "Unable to write crate to file: {}",
                file_path.to_string_lossy()
            )
        })?;

        let sha256 = match self.calc_sha256(&file_path).await {
            Ok(s) => s,
            Err(e) => return Err(e),
        };

        Ok(sha256)
    }

    pub fn crate_path(&self, name: &str, version: &str) -> PathBuf {
        self.crate_folder
            .join(format!("{}-{}.crate", name, version))
    }

    async fn calc_sha256(&self, file_path: &Path) -> Result<String> {
        let mut tries = 0;
        let max_tries = 5;

        while tries < max_tries {
            let mut file = File::open(&file_path)
                .await
                .with_context(|| "Unable to open crate file to calc cksum")?;

            let mut buf: Vec<u8> = vec![];

            let bytes_read = file
                .read_to_end(&mut buf)
                .await
                .with_context(|| "Unable to read crate file to calc cksum")?;

            let real_length = file.metadata().await?.len();
            if bytes_read != real_length as usize {
                let error_msg = format!(
                    "Try {} - Unable to read crate file {} to calc cksum. Read {} bytes, but file length is {}",
                    tries + 1,
                    file_path.display(),
                    bytes_read,
                    real_length
                );
                warn!(error_msg);
                tries += 1;
            } else {
                let sha256: String = Sha256::digest(&buf).encode_hex();
                return Ok(sha256);
            }
        }

        let error_msg = format!(
            "Unable to read crate file {} to calc cksum after {} tries",
            file_path.display(),
            max_tries
        );
        error!(error_msg);
        bail!(error_msg)
    }

    async fn create_bin_path(crate_path: &Path) -> Result<()> {
        if !crate_path.exists() {
            DirBuilder::new()
                .recursive(true)
                .create(&crate_path)
                .await
                .with_context(|| format!("Unable to create bin path: {crate_path:?}"))?;
        }
        Ok(())
    }

    pub async fn get_file(&self, file_path: PathBuf) -> Option<Vec<u8>> {
        async fn from_cache(cache: &CrateCache, file_path: PathBuf) -> Option<Vec<u8>> {
            match cache.get(&file_path).await {
                None => {
                    let mut file = File::open(&file_path).await.ok()?;
                    let mut krate = Vec::new();
                    file.read_to_end(&mut krate).await.ok()?;
                    cache.insert(file_path, krate.clone()).await;
                    Some(krate)
                }
                Some(krate) => Some(krate.to_owned()),
            }
        }

        match &self.cache {
            None => {
                let mut file = File::open(&file_path).await.ok()?;
                let mut krate = Vec::new();
                file.read_to_end(&mut krate).await.ok()?;
                Some(krate)
            }
            Some(c) => from_cache(c, file_path).await,
        }
    }

    pub async fn create_rand_doc_queue_path(&self) -> Result<PathBuf> {
        let rand = generate_rand_string(10);
        let dir = self.doc_queue_path.join(rand);
        Self::create_recursive_path(&dir).await?;

        Ok(dir)
    }

    async fn create_recursive_path(path: &Path) -> Result<()> {
        if !path.exists() {
            DirBuilder::new()
                .recursive(true)
                .create(path)
                .await
                .with_context(|| "Unable to create doc queue sub-path.")?;
        }
        Ok(())
    }
}
