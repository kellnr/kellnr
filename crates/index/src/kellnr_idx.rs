use super::config_json::ConfigJson;
use super::rwindex::{RoIndex, WoIndex};
use super::{common_idx, git};
use crate::git::configure_repo;
use crate::rwindex::RwIndex;
use anyhow::{bail, Context, Result};
use common::index_metadata::{metadata_path, IndexMetadata};
use common::prefetch::Prefetch;
use common::storage_provider::StorageProvider;
use common::version;
use rocket::async_trait;
use rocket::tokio::fs::{remove_file, DirBuilder, File};
use rocket::tokio::io::AsyncWriteExt;
use settings::{Protocol, Settings};
use std::convert::TryFrom;
use std::path::{Path, PathBuf};

pub struct KellnrIdx<T: StorageProvider> {
    index_path: PathBuf,
    api_address: String,
    api_port_proxy: u16,
    protocol: Protocol,
    auth_required: bool,
    storage: T,
}

impl<T: StorageProvider> RwIndex for KellnrIdx<T> {}

#[async_trait]
impl<T: StorageProvider> WoIndex for KellnrIdx<T> {
    async fn add_to_index(&self, metadata: &IndexMetadata) -> Result<()> {
        self.add_metadata(metadata).await?;
        let msg = format!("Add {} - {}", &metadata.name, &metadata.vers);
        git::add_and_commit(&self.index_path, &msg).await?;
        Ok(())
    }

    async fn yank(&self, crate_name: &str, version: &str, yanked: bool) -> Result<()> {
        let metadata_path = metadata_path(&self.index_path, crate_name);
        let mut lines: Vec<IndexMetadata>;
        {
            let mut file = self.storage.open_file(&metadata_path).await?;
            let content = self.storage.read_file(&mut file).await?;
            lines = content
                .lines()
                .filter_map(|line| serde_json::from_str::<IndexMetadata>(line).ok())
                .collect();
        }

        let old_meta = match lines.iter().find(|m| m.vers == version) {
            Some(m) => m,
            None => bail!(
                "Version {} for crate {} does not exist.",
                &version,
                &crate_name
            ),
        };

        let new_meta = IndexMetadata {
            yanked,
            ..old_meta.clone()
        };

        // keep all lines except the version we want to (un)yank and add the (un)yanked version.
        lines.retain(|m| m.vers != version);
        lines.push(new_meta);

        self.overwrite_metadata(&mut lines, &metadata_path).await?;

        let msg = format!("Yanked {} version {}", crate_name, version);
        git::add_and_commit(&self.index_path, &msg).await?;

        Ok(())
    }

    async fn delete(&self, crate_name: &str, version: &str) -> Result<()> {
        let metadata_path = metadata_path(&self.index_path, crate_name);
        let mut lines: Vec<IndexMetadata>;
        {
            let mut file = self.storage.open_file(&metadata_path).await?;
            let content = self.storage.read_file(&mut file).await?;
            lines = content
                .lines()
                .filter_map(|line| serde_json::from_str::<IndexMetadata>(line).ok())
                .collect();
        }

        // Keep all lines except the one with the version we want to delete
        lines.retain(|m| m.vers != version);

        // If it was the last version, delete the whole crate file so there is no empty file on disk
        if lines.is_empty() {
            rocket::tokio::fs::remove_file(&metadata_path).await?;
        } else {
            self.overwrite_metadata(&mut lines, &metadata_path).await?;
        }

        let msg = format!("Deleted {} version {}", crate_name, version);
        git::add_and_commit(&self.index_path, &msg).await?;
        Ok(())
    }
}

#[async_trait]
impl<T: StorageProvider> RoIndex for KellnrIdx<T> {
    fn get_config(&self) -> ConfigJson {
        ConfigJson::new(
            &self.protocol,
            &self.api_address,
            self.api_port_proxy,
            "crates",
            self.auth_required,
        )
    }

    async fn get_prefetch_data(&self, package: &str) -> Result<Prefetch> {
        common_idx::get_prefetch_data(package, &self.index_path, &self.storage).await
    }
}

impl<T: StorageProvider> KellnrIdx<T> {
    pub async fn new(settings: &Settings, storage: T) -> Result<Self> {
        let idx = Self {
            index_path: settings.index_path(),
            api_address: settings.api_address.clone(),
            api_port_proxy: settings.api_port_proxy,
            protocol: settings.api_protocol,
            auth_required: settings.auth_required,
            storage,
        };

        if settings.git_index {
            idx.create_index_repo().await?;
        }

        Ok(idx)
    }

    async fn create_index_repo(&self) -> Result<()> {
        if !self.repo_exits() {
            self.create_repo().await?;
        }

        if !self.is_initialized() {
            git::init(&self.index_path)?;
            self.initial_commit().await?;
        }

        configure_repo(&self.index_path)?;
        common_idx::update_config_json(&self.get_config(), &self.index_path).await?;
        common_idx::update_export_file(&self.index_path).await?;
        Ok(())
    }

    async fn crate_version_exists(&self, file: &mut File, vers: &str) -> Result<bool> {
        let content = self.storage.read_file(file).await?;

        Ok(content
            .lines()
            .filter_map(|line| serde_json::from_str::<IndexMetadata>(line).ok())
            .any(|m| m.vers == vers))
    }

    fn is_initialized(&self) -> bool {
        self.index_path.join(".git").join("config").exists()
    }

    fn repo_exits(&self) -> bool {
        self.index_path.exists()
    }

    async fn create_repo(&self) -> Result<()> {
        DirBuilder::new()
            .recursive(true)
            .create(&self.index_path)
            .await
            .with_context(|| {
                format!(
                    "Unable to create directory for index: {}",
                    &self.index_path.display()
                )
            })?;
        Ok(())
    }

    async fn initial_commit(&self) -> Result<()> {
        let file_path = self.index_path.join("Readme.md");
        let file_content = "kellnr index. Do not delete.";

        git::add_file_and_commit(
            &file_path,
            file_content,
            &self.index_path,
            "Kellnr index. Do not delete",
        )
        .await
    }

    async fn overwrite_metadata(
        &self,
        data: &mut Vec<IndexMetadata>,
        metadata_path: &Path,
    ) -> Result<()> {
        if metadata_path.exists() {
            remove_file(&metadata_path)
                .await
                .with_context(|| "Unable to delete metadata file.")?;
        } else {
            let file_dir = metadata_path.parent().unwrap(); // safe to unwrap as there is always a parent.

            if !file_dir.exists() {
                DirBuilder::new()
                    .recursive(true)
                    .create(file_dir)
                    .await
                    .with_context(|| "Unable to create index sub-path.")?;
            }
        }

        let mut file = self.storage.open_or_create_file(metadata_path).await?;

        data.sort_by(|a, b| {
            version::Version::try_from(&a.vers)
                .unwrap()
                .cmp(&version::Version::try_from(&b.vers).unwrap())
        });

        for metadata in data {
            let metadata_json = &metadata
                .to_json()
                .with_context(|| "Unable to convert metadata to json")?;

            file.write(format!("{}\n", metadata_json).as_bytes())
                .await
                .with_context(|| "Unable to append metadata to index file.")?;
        }

        Ok(())
    }

    async fn add_metadata(&self, metadata: &IndexMetadata) -> Result<()> {
        let file_path = metadata.metadata_path(&self.index_path);
        let file_dir = &file_path.parent().unwrap(); // safe to unwrap as there is always a parent.

        if !file_dir.exists() {
            DirBuilder::new()
                .recursive(true)
                .create(file_dir)
                .await
                .with_context(|| "Unable to create index sub-path.")?;
        }

        let mut file = self.storage.open_or_create_file(&file_path).await?;

        if self.crate_version_exists(&mut file, &metadata.vers).await? {
            bail!(
                "Crate with same version already exists. Crate: {} - Version: {}",
                &metadata.name,
                &metadata.vers
            )
        }

        let metadata_json = &metadata
            .to_json()
            .with_context(|| "Unable to convert metadata to json")?;

        file.write(format!("{}\n", &metadata_json).as_bytes())
            .await
            .with_context(|| "Unable to append metadata to index file.")?;

        Ok(())
    }
}

#[cfg(test)]
mod kellnr_idx_tests {
    use super::*;
    use common::storage::Storage;
    use rocket::async_test;

    struct TestIndex {
        index_path: PathBuf,
        data_dir: PathBuf,
        idx: KellnrIdx<Storage>,
    }

    impl TestIndex {
        async fn from(data_dir: &str) -> TestIndex {
            let settings = Settings {
                data_dir: "/tmp/".to_string() + data_dir,
                ..Settings::new().unwrap()
            };
            let storage = Storage::new();
            let idx = KellnrIdx::new(&settings, storage)
                .await
                .expect("Cannot create test index directory.");

            TestIndex {
                index_path: settings.index_path(),
                data_dir: PathBuf::from(settings.data_dir),
                idx,
            }
        }
    }

    impl Drop for TestIndex {
        fn drop(&mut self) {
            rm_rf::remove(&self.data_dir).expect("Cannot remove test index directory.");
        }
    }

    #[async_test]
    async fn get_prefetch_data_returns_metadata_byte_vec() {
        let test_index: TestIndex =
            TestIndex::from("get_prefetch_data_returns_metadata_byte_vec").await;
        test_index
            .idx
            .add_to_index(&IndexMetadata::minimal("foobar", "0.10.0", "sha256"))
            .await
            .expect("Cannot add metadata to index");

        let prefetch = test_index.idx.get_prefetch_data("foobar").await.unwrap();

        assert_eq!(
            "fe74c3d1a6669ad3959c62236bbe0e6f6e046cb998ee18a59916d83c43a786de",
            prefetch.etag
        );
        assert_eq!(96, prefetch.data.len());
        //assert!(prefetch.last_modified.);
    }

    #[async_test]
    async fn yank_version() {
        let test_index = TestIndex::from("test_yank_version").await;

        // Fill the index with two versions
        let metadata1 = IndexMetadata::minimal("test", "0.10.0", "sha256");
        let metadata2 = IndexMetadata::minimal("test", "0.20.0", "sha256");

        test_index
            .idx
            .add_to_index(&metadata1)
            .await
            .expect("Cannot add metadata to index");
        test_index
            .idx
            .add_to_index(&metadata2)
            .await
            .expect("Cannot add metadata to index");

        // Yank one version and check the result
        let result = test_index.idx.yank("test", "0.10.0", true).await;
        assert!(result.is_ok());

        // Check that the version was yanked in the metadata file.
        let mut file = test_index
            .idx
            .storage
            .open_file(&metadata2.metadata_path(&test_index.index_path))
            .await
            .expect("Failed to open metadata file.");
        let content = test_index
            .idx
            .storage
            .read_file(&mut file)
            .await
            .expect("Failed to read metadata file.");
        let lines: Vec<IndexMetadata> = content
            .lines()
            .filter_map(|line| serde_json::from_str::<IndexMetadata>(line).ok())
            .collect();
        assert!(lines.iter().any(|m| m.vers == "0.10.0" && m.yanked));
    }

    #[async_test]
    async fn unyank_version() {
        let test_index = TestIndex::from("test_unyank_version").await;

        // Fill the index with two versions. One is yanked.
        let mut metadata1 = IndexMetadata::minimal("test", "0.10.0", "sha256");
        metadata1.yanked = true;
        let metadata2 = IndexMetadata::minimal("test", "0.20.0", "sha256");

        test_index
            .idx
            .add_to_index(&metadata1)
            .await
            .expect("Cannot add metadata to index");
        test_index
            .idx
            .add_to_index(&metadata2)
            .await
            .expect("Cannot add metadata to index");

        // Unyank one version and check the result
        let result = test_index.idx.yank("test", "0.10.0", false).await;
        assert!(result.is_ok());

        // Check that the version was yanked in the metadata file.
        let mut file = test_index
            .idx
            .storage
            .open_file(&metadata2.metadata_path(&test_index.index_path))
            .await
            .expect("Failed to open metadata file.");
        let content = test_index
            .idx
            .storage
            .read_file(&mut file)
            .await
            .expect("Failed to read metadata file.");
        let lines: Vec<IndexMetadata> = content
            .lines()
            .filter_map(|line| serde_json::from_str::<IndexMetadata>(line).ok())
            .collect();
        assert!(lines.iter().any(|m| m.vers == "0.10.0" && !m.yanked));
    }

    #[async_test]
    async fn add_file_to_index() {
        let test_index = TestIndex::from("test_add_file_to_index").await;
        let metadata = IndexMetadata::minimal("test", "0.10.0", "sha256");

        let result = test_index.idx.add_to_index(&metadata).await;

        assert!(result.is_ok());
    }

    #[async_test]
    async fn add_duplicate_file_to_index() {
        let test_index = TestIndex::from("test_add_duplicate_file_to_index").await;
        let metadata = IndexMetadata::minimal("test", "0.10.0", "sha256");

        let _ = test_index.idx.add_to_index(&metadata).await;
        let result = test_index.idx.add_to_index(&metadata).await;

        assert!(result.is_err());
        assert_eq!(
            "Crate with same version already exists. Crate: test - Version: 0.10.0",
            result.unwrap_err().to_string()
        );
    }

    #[async_test]
    async fn add_multiple_versions_to_index() {
        let test_index = TestIndex::from("test_add_multiple_versions_to_index").await;
        let metadata1 = IndexMetadata::minimal("test", "0.1.0", "sha256");
        let metadata2 = IndexMetadata::minimal("test", "0.2.0", "sha256");

        let result1 = test_index.idx.add_to_index(&metadata1).await;
        let result2 = test_index.idx.add_to_index(&metadata2).await;

        assert!(result1.is_ok());
        assert!(result2.is_ok());
    }

    #[async_test]
    async fn add_multiple_crates_to_index() {
        let test_index = TestIndex::from("test_add_multiple_crates_to_index").await;
        let metadata1 = IndexMetadata::minimal("test1", "0.1.0", "sha256");
        let metadata2 = IndexMetadata::minimal("test2", "0.1.0", "sha256");

        let result1 = test_index.idx.add_to_index(&metadata1).await;
        let result2 = test_index.idx.add_to_index(&metadata2).await;

        assert!(result1.is_ok());
        assert!(result2.is_ok());
    }

    #[async_test]
    async fn delete_one_of_many_versions() {
        let test_index = TestIndex::from("test_delete_one_of_many_versions").await;

        // Fill the index with two versions
        let metadata1 = IndexMetadata::minimal("test", "0.10.0", "sha256");
        let metadata2 = IndexMetadata::minimal("test", "0.20.0", "sha256");
        let metadata3 = IndexMetadata::minimal("test", "0.30.0", "sha256");

        test_index
            .idx
            .add_to_index(&metadata1)
            .await
            .expect("Cannot add metadata to index");
        test_index
            .idx
            .add_to_index(&metadata2)
            .await
            .expect("Cannot add metadata to index");
        test_index
            .idx
            .add_to_index(&metadata3)
            .await
            .expect("Cannot add metadata to index");

        // Delete one version and check the result
        test_index.idx.delete("test", "0.20.0").await.unwrap();

        // Check that the version was deleted in the metadata file.
        let mut file = test_index
            .idx
            .storage
            .open_file(&metadata2.metadata_path(&test_index.index_path))
            .await
            .expect("Failed to open metadata file.");
        let content = test_index
            .idx
            .storage
            .read_file(&mut file)
            .await
            .expect("Failed to read metadata file.");
        let lines: Vec<IndexMetadata> = content
            .lines()
            .filter_map(|line| serde_json::from_str::<IndexMetadata>(line).ok())
            .collect();

        assert_eq!(2, lines.len());
        assert_eq!("0.10.0", lines[0].vers);
        assert_eq!("0.30.0", lines[1].vers);
    }

    #[async_test]
    async fn delete_only_version() {
        let test_index = TestIndex::from("test_delete_only_version").await;
        let metadata = IndexMetadata::minimal("test", "0.10.0", "sha256");
        test_index
            .idx
            .add_to_index(&metadata)
            .await
            .expect("Cannot add metadata to index");

        // Delete one version and check the result
        test_index.idx.delete("test", "0.10.0").await.unwrap();

        assert!(!metadata.metadata_path(&test_index.index_path).exists());
    }
}
