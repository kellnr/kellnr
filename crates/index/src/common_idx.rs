use super::config_json::ConfigJson;
use super::git;
use anyhow::{Context, Result};
use common::index_metadata::metadata_path;
use common::prefetch::Prefetch;
use common::storage_provider::StorageProvider;
use rocket::tokio::fs::{self, File};
use std::path::Path;
use tracing::log::debug;

pub async fn get_prefetch_data(
    package: &str,
    index_path: &Path,
    storage: &impl StorageProvider,
) -> Result<Prefetch> {
    let file = metadata_path(index_path, package);
    let data = storage.read(&file).await?;
    let last_modified = format!("{:?}", file.metadata()?.modified()?);

    use hex::ToHex;
    use sha2::Digest;
    let hash_bytes = sha2::Sha256::digest(&data);
    let etag = (&hash_bytes[..]).encode_hex::<String>();

    Ok(Prefetch {
        data,
        etag,
        last_modified,
    })
}

pub async fn update_config_json(config: &ConfigJson, index_path: &Path) -> Result<()> {
    let config_path = index_path.join("config.json");

    if let Ok(conf_string) = fs::read_to_string(&config_path).await {
        if let Ok(current_conf) = serde_json::from_str::<ConfigJson>(&conf_string) {
            if &current_conf == config {
                return Ok(());
            }
        }
    }

    debug!("Update config.json: {:?}", config);
    add_config_json(config, index_path).await
}

pub async fn update_export_file(index_path: &Path) -> Result<()> {
    if Path::exists(&index_path.join(".git").join("git-daemon-export-ok")) {
        Ok(())
    } else {
        add_export_file(index_path).await
    }
}

async fn add_config_json(config: &ConfigJson, index_path: &Path) -> Result<()> {
    let file_content = config.to_json()?;
    let file_path = Path::new(&index_path).join("config.json");
    git::add_file_and_commit(&file_path, &file_content, index_path, "Add config.json").await
}

async fn add_export_file(index_path: &Path) -> Result<()> {
    let _ = File::create(index_path.join(".git").join("git-daemon-export-ok"))
        .await
        .with_context(|| "Unable to create git daemon export file.")?;
    Ok(())
}
