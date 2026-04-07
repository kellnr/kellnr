use sea_orm_migration::prelude::*;

use crate::iden::{ToolchainComponentIden, ToolchainTargetIden};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add status column to toolchain_target (defaults to "ready" for existing rows)
        manager
            .alter_table(
                Table::alter()
                    .table(ToolchainTargetIden::Table)
                    .add_column(
                        ColumnDef::new(ToolchainTargetIden::Status)
                            .text()
                            .not_null()
                            .default("ready"),
                    )
                    .to_owned(),
            )
            .await?;

        // toolchain_component table for individual component archives
        manager
            .create_table(
                Table::create()
                    .table(ToolchainComponentIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ToolchainComponentIden::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ToolchainComponentIden::ToolchainTargetFk)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ToolchainComponentIden::Name)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ToolchainComponentIden::StoragePath)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ToolchainComponentIden::Hash)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ToolchainComponentIden::Size)
                            .big_integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("toolchain_component_target_fk")
                            .from(
                                ToolchainComponentIden::Table,
                                ToolchainComponentIden::ToolchainTargetFk,
                            )
                            .to(ToolchainTargetIden::Table, ToolchainTargetIden::Id)
                            .on_update(ForeignKeyAction::NoAction)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_toolchain_component_unique")
                    .table(ToolchainComponentIden::Table)
                    .col(ToolchainComponentIden::ToolchainTargetFk)
                    .col(ToolchainComponentIden::Name)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(ToolchainComponentIden::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(ToolchainTargetIden::Table)
                    .drop_column(ToolchainTargetIden::Status)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
