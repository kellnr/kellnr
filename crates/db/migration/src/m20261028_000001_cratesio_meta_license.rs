use sea_orm_migration::prelude::*;

use crate::iden::CratesIoMetaIden;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // SPDX license expression of a cached crates.io version. NULL means it
        // was not fetched yet, which is what all existing versions start with.
        // They get it on the next index sync of their crate.
        manager
            .alter_table(
                Table::alter()
                    .table(CratesIoMetaIden::Table)
                    .add_column(ColumnDef::new(CratesIoMetaIden::License).text())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(CratesIoMetaIden::Table)
                    .drop_column(CratesIoMetaIden::License)
                    .to_owned(),
            )
            .await
    }
}
