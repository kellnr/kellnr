use super::config_json::ConfigJson;
use super::rwindex::RoIndex;
use super::{common_idx, git};
use anyhow::Result;
use common::prefetch::Prefetch;
use common::storage_provider::StorageProvider;
use rocket::async_trait;
use rocket::tokio::fs::DirBuilder;
use settings::{Protocol, Settings};
use std::path::{Path, PathBuf};
use tracing::{debug, error, info, warn};

pub struct CratesIoIdx<T: StorageProvider> {
    index_path: PathBuf,
    api_address: String,
    api_port_proxy: u16,
    protocol: Protocol,
    auth_required: bool,
    storage: T,
}

#[async_trait]
impl<T: StorageProvider> RoIndex for CratesIoIdx<T> {
    fn get_config(&self) -> ConfigJson {
        ConfigJson::new(
            &self.protocol,
            &self.api_address,
            self.api_port_proxy,
            "cratesio",
            self.auth_required,
        )
    }

    async fn get_prefetch_data(&self, package: &str) -> Result<Prefetch> {
        common_idx::get_prefetch_data(package, &self.index_path, &self.storage).await
    }
}

impl<T: StorageProvider> CratesIoIdx<T> {
    pub fn new(settings: &Settings, storage: T) -> Self {
        Self {
            index_path: settings.crates_io_index_path(),
            api_address: settings.api_address.clone(),
            api_port_proxy: settings.api_port_proxy,
            protocol: settings.api_protocol,
            auth_required: settings.auth_required,
            storage,
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        init_repo(&self.index_path, &self.get_config()).await?;
        self.start_pull_loop();
        Ok(())
    }

    fn start_pull_loop(&self) {
        let index_path = self.index_path.clone();
        let config_json = self.get_config();
        rocket::tokio::spawn(async move {
            loop {
                pull_loop(&index_path, 60).await;
                if let Err(e) = init_repo(&index_path, &config_json).await {
                    error!("Failed to init crates.io index: {}", e);
                }
            }
        });
    }
}

async fn init_repo(index_path: &Path, config_json: &ConfigJson) -> Result<()> {
    if !git::repo_exists(index_path) {
        info!("Crates.io index does not exist. Start cloning the index.");
        info!("This can take some while...");

        DirBuilder::new().recursive(true).create(index_path).await?;

        git::clone_repo(index_path, "https://github.com/rust-lang/crates.io-index")?;
        info!("Cloning done.");
    } else {
        info!("Crates.io index already exists.");
    }
    if git::is_locked(index_path) {
        warn!("Crates.io index is locked. Try to unlock.");
        git::unlock(index_path).await?;
    }
    git::configure_repo(index_path)?;
    common_idx::update_config_json(config_json, index_path).await?;
    common_idx::update_export_file(index_path).await?;
    Ok(())
}

async fn pull_loop(repo_path: &Path, pull_interval: u64) {
    debug!("Started update loop for repository.");

    let mut err_cnt = 0;

    loop {
        rocket::tokio::time::sleep(std::time::Duration::from_secs(pull_interval)).await;

        if !repo_path.exists() {
            break;
        }

        if err_cnt >= 30 {
            remove_cratesio_index(repo_path.into()).await;
            break;
        }

        if let Err(e) =
            git::add_and_commit(repo_path, "Commit all changes before pull of origin").await
        {
            err_cnt += 1;
            warn!("Failed to add and commit index: {e}");
        }

        if let Err(e) = git::pull(repo_path) {
            err_cnt += 1;
            warn!("Failed to pull and merge remote branch: {e}");
        }
    }
}

pub async fn remove_cratesio_index(repo_path: PathBuf) {
    error!("Too many crates.io related error. Repairing git repository...");
    match rocket::tokio::task::spawn_blocking(move || {
        if let Err(e) = rm_rf::ensure_removed(repo_path) {
            error!("Failed to remove repository: {}", e);
        }
    })
    .await
    {
        Ok(_) => info!("Repository removed."),
        Err(e) => {
            error!("Failed to remove repository: {}", e);
        }
    };
}
