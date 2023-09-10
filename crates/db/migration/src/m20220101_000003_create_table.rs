use crate::sea_orm::ActiveValue::Set;
use crate::sea_orm::{ActiveModelTrait, EntityTrait};
use chrono::NaiveDateTime;
use common::version::Version;
use sea_orm_migration::prelude::*;
use tracing::debug;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Manual check if the column exists is needed, as Sqlite does not support
        // ALTER TABLE IF COLUMN EXISTS. Without the check, the migration would fail
        // on Sqlite with an "duplicate column" error.
        if manager.has_column("krate", "max_version").await? {
            debug!("Column krate.max_version already exists");
        } else {
            manager
                .alter_table(
                    Table::alter()
                        .table(CrateIden::Table)
                        .add_column_if_not_exists(
                            ColumnDef::new(CrateIden::MaxVersion)
                                .text()
                                .not_null()
                                .default(""),
                        )
                        .to_owned(),
                )
                .await?;
            debug!("Added new column max_version");
        }

        if manager.has_column("krate", "total_downloads").await? {
            debug!("Column krate.total_downloads already exists");
        } else {
            manager
                .alter_table(
                    Table::alter()
                        .table(CrateIden::Table)
                        .add_column_if_not_exists(
                            ColumnDef::new(CrateIden::TotalDownloads)
                                .big_integer()
                                .not_null()
                                .default(0),
                        )
                        .to_owned(),
                )
                .await?;
            debug!("Added new column total_downloads");
        }

        if manager.has_column("krate", "last_updated").await? {
            debug!("Column krate.last_updated already exists");
        } else {
            manager
                .alter_table(
                    Table::alter()
                        .table(CrateIden::Table)
                        .add_column_if_not_exists(
                            ColumnDef::new(CrateIden::LastUpdated)
                                .text()
                                .not_null()
                                .default(""),
                        )
                        .to_owned(),
                )
                .await?;
            debug!("Added new column last_updated");
        }

        fill_new_columns(manager.get_connection()).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(CrateIden::Table)
                    .drop_column(CrateIden::MaxVersion)
                    .drop_column(CrateIden::TotalDownloads)
                    .drop_column(CrateIden::LastUpdated)
                    .to_owned(),
            )
            .await
    }
}

async fn fill_new_columns(db: &SchemaManagerConnection<'_>) -> Result<(), DbErr> {
    debug!("Filling new columns");
    use crate::m20220101_000003_create_table_entities::{crate_meta, krate};

    let crates: Vec<(krate::Model, Vec<crate_meta::Model>)> = krate::Entity::find()
        .find_with_related(crate_meta::Entity)
        .all(db)
        .await?;

    for (krate, metas) in crates {
        let default_version = Version::default().to_string();
        let max_version = metas
            .iter()
            .max_by_key(|m| Version::try_from(&m.version).unwrap_or_default())
            .map(|m| &m.version)
            .unwrap_or(&default_version);

        let total_downloads: i64 = metas.iter().map(|m| m.downloads).sum();

        let last_updated = metas
            .iter()
            .max_by_key(|m| {
                NaiveDateTime::parse_from_str(&m.created, "%Y-%m-%d %H:%M:%S").unwrap_or_default()
            })
            .map_or("2000-01-01 00:00:00", |m| &m.created);

        let k: Option<krate::Model> = krate::Entity::find_by_id(krate.id).one(db).await?;
        let mut k: krate::ActiveModel = k.unwrap().into();
        k.max_version = Set(max_version.to_owned());
        k.total_downloads = Set(total_downloads);
        k.last_updated = Set(last_updated.to_owned());
        k.update(db).await?;
    }
    debug!("Filled new columns");
    Ok(())
}

#[derive(Iden)]
pub enum CrateIden {
    #[iden = "krate"]
    Table,
    MaxVersion,
    TotalDownloads,
    LastUpdated,
}
