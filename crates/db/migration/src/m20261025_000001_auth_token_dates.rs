use sea_orm_migration::prelude::*;

use crate::iden::AuthTokenIden;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // When a token was created and last used. Both are unknown for
        // existing tokens and stay empty until the token is used.
        manager
            .alter_table(
                Table::alter()
                    .table(AuthTokenIden::Table)
                    .add_column(ColumnDef::new(AuthTokenIden::Created).text())
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(AuthTokenIden::Table)
                    .add_column(ColumnDef::new(AuthTokenIden::LastUsed).text())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(AuthTokenIden::Table)
                    .drop_column(AuthTokenIden::LastUsed)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(AuthTokenIden::Table)
                    .drop_column(AuthTokenIden::Created)
                    .to_owned(),
            )
            .await
    }
}
