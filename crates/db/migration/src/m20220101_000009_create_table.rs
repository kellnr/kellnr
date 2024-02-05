use crate::sea_orm::ActiveValue::Set;
use crate::sea_orm::{ActiveModelTrait, EntityTrait};
use sea_orm::{ModelTrait, Related};
use sea_orm_migration::prelude::*;
use settings::{get_settings, Settings};
use tracing::debug;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        move_cached_crates(manager.get_connection()).await
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}

async fn move_cached_crates(db: &SchemaManagerConnection<'_>) -> Result<(), DbErr> {
    use crate::m20220101_000009_create_table_entities::{cratesio_crate, cratesio_index};

    debug!("Moving cached crates...");
    let settings = get_settings().map_err(|e| DbErr::Custom(e.to_string()))?;

    // Make sure the cratesio bin path exists
    if !settings.crates_io_bin_path().exists() {
        std::fs::create_dir_all(settings.crates_io_bin_path())
            .map_err(|e| DbErr::Custom(e.to_string()))?;
    }

    // Get all cached crate versions
    let cached_indicies = cratesio_index::Entity::find()
        .all(db)
        .await?
        .into_iter()
        .collect::<Vec<_>>();

    // Move each crate to the new location
    for cached_index in cached_indicies {
        let cached_crate = cached_index
            .find_related(cratesio_crate::Entity)
            .one(db)
            .await?;
        let name = cached_crate
            .ok_or(DbErr::Custom("No crate found".to_string()))?
            .original_name;
        let (old, new) = get_path(&settings, &name, &cached_index.vers);

        if !old.exists() {
            // crate is in the database index but not on disk. Skip the move.
            continue;
        }

        // Move the crate
        debug!("Moving {} from {:?} to {:?}", name, old, new);
        if let Err(e) = std::fs::rename(old, new).map_err(|e| DbErr::Custom(e.to_string())) {
            debug!("Failed to move {}: {}", name, e);
            continue;
        }
    }
    debug!("Done moving cached crates");
    Ok(())
}

fn get_path(
    settings: &Settings,
    name: &str,
    version: &str,
) -> (std::path::PathBuf, std::path::PathBuf) {
    (
        settings
            .bin_path()
            .join(format!("{}-{}.crate", name, version)),
        settings
            .crates_io_bin_path()
            .join(format!("{}-{}.crate", name, version)),
    )
}
