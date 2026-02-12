//! Migration for toolchain distribution server tables
//!
//! This migration adds tables for:
//! - toolchain: Stores toolchain releases (e.g., "rust 1.75.0")
//! - toolchain_target: Stores target-specific archives for each toolchain

use sea_orm_migration::prelude::*;

use crate::iden::{ToolchainIden, ToolchainTargetIden};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // toolchain table - stores toolchain releases
        manager
            .create_table(
                Table::create()
                    .table(ToolchainIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ToolchainIden::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ToolchainIden::Name).text().not_null())
                    .col(ColumnDef::new(ToolchainIden::Version).text().not_null())
                    .col(ColumnDef::new(ToolchainIden::Date).text().not_null())
                    .col(ColumnDef::new(ToolchainIden::Channel).text())
                    .col(ColumnDef::new(ToolchainIden::Created).text().not_null())
                    .to_owned(),
            )
            .await?;

        // Unique index on (name, version)
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_toolchain_name_version")
                    .table(ToolchainIden::Table)
                    .col(ToolchainIden::Name)
                    .col(ToolchainIden::Version)
                    .unique()
                    .to_owned(),
            )
            .await?;

        // Index on channel for efficient lookups
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_toolchain_channel")
                    .table(ToolchainIden::Table)
                    .col(ToolchainIden::Channel)
                    .to_owned(),
            )
            .await?;

        // toolchain_target table - stores target-specific archives
        manager
            .create_table(
                Table::create()
                    .table(ToolchainTargetIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ToolchainTargetIden::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ToolchainTargetIden::ToolchainFk)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ToolchainTargetIden::Target)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ToolchainTargetIden::StoragePath)
                            .text()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ToolchainTargetIden::Hash).text().not_null())
                    .col(
                        ColumnDef::new(ToolchainTargetIden::Size)
                            .big_integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("toolchain_target_toolchain_fk")
                            .from(ToolchainTargetIden::Table, ToolchainTargetIden::ToolchainFk)
                            .to(ToolchainIden::Table, ToolchainIden::Id)
                            .on_update(ForeignKeyAction::NoAction)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Unique index on (toolchain_fk, target)
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_toolchain_target_unique")
                    .table(ToolchainTargetIden::Table)
                    .col(ToolchainTargetIden::ToolchainFk)
                    .col(ToolchainTargetIden::Target)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop tables in reverse order (respecting foreign key constraints)
        manager
            .drop_table(Table::drop().table(ToolchainTargetIden::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(ToolchainIden::Table).to_owned())
            .await?;

        Ok(())
    }
}
