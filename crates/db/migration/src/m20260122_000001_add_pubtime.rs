use sea_orm_migration::prelude::*;
use sea_orm_migration::schema::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Manual check if the column exists is needed, as Sqlite does not support
        // ALTER TABLE IF COLUMN EXISTS. Without the check, the migration would fail
        // on Sqlite with an "duplicate column" error.

        if !manager.has_column("crate_index", "pubtime").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(CrateIndexIden::Table)
                        .add_column_if_not_exists(
                            ColumnDef::new(CrateIndexIden::Pubtime).date_time(),
                        )
                        .to_owned(),
                )
                .await?;
        }

        if !manager.has_column("cratesio_index", "pubtime").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(CratesIoIndexIden::Table)
                        .add_column_if_not_exists(
                            ColumnDef::new(CratesIoIndexIden::Pubtime).date_time(),
                        )
                        .to_owned(),
                )
                .await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(CrateIndexIden::Table)
                    .drop_column(CrateIndexIden::Pubtime)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(CratesIoIndexIden::Table)
                    .drop_column(CratesIoIndexIden::Pubtime)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
pub enum CrateIndexIden {
    #[iden = "crate_index"]
    Table,
    Id,
    Vers,
    Deps,
    Cksum,
    Features,
    Yanked,
    Links,
    Pubtime,
    V,
    CrateFk,
}

#[derive(Iden)]
pub enum CratesIoIndexIden {
    #[iden = "cratesio_index"]
    Table,
    Id,
    Vers,
    Deps,
    Cksum,
    Features,
    Yanked,
    Links,
    Pubtime,
    V,
    CratesIoFk,
}
