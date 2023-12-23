use sea_orm_migration::prelude::*;
use crate::sea_orm::ActiveValue::Set;
use crate::sea_orm::{ActiveModelTrait, EntityTrait};
use tracing::debug;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Manual check if the column exists is needed, as Sqlite does not support
        // ALTER TABLE IF COLUMN EXISTS. Without the check, the migration would fail
        // on Sqlite with an "duplicate column" error.

        if !manager
            .has_column("cratesio_crate", "total_downloads")
            .await?
        {
            manager
                .alter_table(
                    Table::alter()
                        .table(CratesIoIden::Table)
                        .add_column_if_not_exists(
                            ColumnDef::new(CratesIoIden::TotalDownloads)
                                .big_integer()
                                .not_null()
                                .default(0),
                        )
                        .to_owned(),
                )
                .await?;
        }

        manager
            .create_table(
                Table::create()
                    .table(CratesIoMetaIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CratesIoMetaIden::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(ColumnDef::new(CratesIoMetaIden::Version).text().not_null())
                    .col(
                        ColumnDef::new(CratesIoMetaIden::Downloads)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(CratesIoMetaIden::CratesIoFk)
                            .big_integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("cratesio_fk")
                            .from(CratesIoMetaIden::Table, CratesIoMetaIden::CratesIoFk)
                            .to(CratesIoIden::Table, CratesIoIden::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await?;

        fill_new_columns(manager.get_connection()).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop tables in reverse order of creation
        manager
            .drop_table(Table::drop().table(CratesIoMetaIden::Table).to_owned())
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(CratesIoIden::Table)
                    .drop_column(CratesIoIden::TotalDownloads)
                    .to_owned(),
            )
            .await
    }
}


async fn fill_new_columns(db: &SchemaManagerConnection<'_>) -> Result<(), DbErr> {
    use crate::m20220101_000008_create_table_entities::{cratesio_index, cratesio_meta};

    // Get all cached crate versions
    let cached_versions = cratesio_index::Entity::find()
        .all(db)
        .await?
        .into_iter()
        .collect::<Vec<_>>();

    // Create a cossepsonding cratesio_meta entry for each cached crate version
    for cached_version in cached_versions {
        let crate_id = cached_version.crates_io_fk;
        let version = cached_version.vers;

        let meta = cratesio_meta::ActiveModel {
            id: Default::default(),
            version: Set(version),
            downloads: Default::default(),
            crates_io_fk: Set(crate_id),
        };

        cratesio_meta::Entity::insert(meta).exec(db).await?;
    }

    Ok(())
}

#[derive(Iden)]
pub enum CratesIoIden {
    #[iden = "cratesio_crate"]
    Table,
    Id,
    TotalDownloads,
}

#[derive(Iden)]
pub enum CratesIoMetaIden {
    #[iden = "cratesio_meta"]
    Table,
    Id,
    Version,
    Downloads,
    CratesIoFk,
}
