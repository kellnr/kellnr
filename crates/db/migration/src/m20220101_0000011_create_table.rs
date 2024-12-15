use sea_orm_migration::prelude::*;

use crate::iden::{CrateIden, CrateUserIden, UserIden};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Manual check if the column exists is needed, as Sqlite does not support
        // ALTER TABLE IF COLUMN EXISTS. Without the check, the migration would fail
        // on Sqlite with an "duplicate column" error.

        if !manager.has_column("krate", "restricted_download").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(CrateIden::Table)
                        .add_column_if_not_exists(
                            ColumnDef::new(CrateIden::RestrictedDownload)
                                .boolean()
                                .not_null()
                                .default(false),
                        )
                        .to_owned(),
                )
                .await?;
        }

        manager
            .create_table(
                Table::create()
                    .table(CrateUserIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CrateUserIden::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(CrateUserIden::CrateFk)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CrateUserIden::UserFk)
                            .big_integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("crate_fk")
                            .from(CrateUserIden::Table, CrateUserIden::CrateFk)
                            .to(CrateIden::Table, CrateIden::Id)
                            .on_update(ForeignKeyAction::NoAction)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("user_fk")
                            .from(CrateUserIden::Table, CrateUserIden::UserFk)
                            .to(UserIden::Table, UserIden::Id)
                            .on_update(ForeignKeyAction::NoAction)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(CrateUserIden::Table).to_owned())
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(CrateIden::Table)
                    .drop_column(CrateIden::RestrictedDownload)
                    .to_owned(),
            )
            .await
    }
}
