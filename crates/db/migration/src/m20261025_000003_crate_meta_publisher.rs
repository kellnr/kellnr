use sea_orm_migration::prelude::*;

use crate::iden::CrateMetaIden;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Name of the user who published a version, and of the auth token used
        // for it. Names instead of foreign keys keep the history readable when
        // a user or token is deleted. Both are unknown for existing versions,
        // and the token is unknown for publishes with basic auth.
        manager
            .alter_table(
                Table::alter()
                    .table(CrateMetaIden::Table)
                    .add_column(ColumnDef::new(CrateMetaIden::Publisher).text())
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(CrateMetaIden::Table)
                    .add_column(ColumnDef::new(CrateMetaIden::PublisherToken).text())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(CrateMetaIden::Table)
                    .drop_column(CrateMetaIden::PublisherToken)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(CrateMetaIden::Table)
                    .drop_column(CrateMetaIden::Publisher)
                    .to_owned(),
            )
            .await
    }
}
