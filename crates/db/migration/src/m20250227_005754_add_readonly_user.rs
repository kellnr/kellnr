use sea_orm_migration::prelude::*;
use sea_orm_migration::schema::*;

use crate::iden::UserIden;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Manual check if the column exists is needed, as Sqlite does not support
        // ALTER TABLE IF COLUMN EXISTS. Without the check, the migration would fail
        // on Sqlite with an "duplicate column" error.

        if !manager.has_column("user", "is_read_only").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(UserIden::Table)
                        .add_column_if_not_exists(
                            ColumnDef::new(UserIden::IsReadOnly)
                                .boolean()
                                .not_null()
                                .default(false),
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
                    .table(UserIden::Table)
                    .drop_column(UserIden::IsReadOnly)
                    .to_owned(),
            )
            .await
    }
}
