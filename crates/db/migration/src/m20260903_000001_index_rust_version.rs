use sea_orm_migration::prelude::*;

use crate::iden::{CrateIndexIden, CratesIoIndexIden};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // The minimal supported Rust version is part of the sparse index
        // format. Without it, cargo's MSRV-aware resolver cannot tell which
        // versions the active toolchain is able to compile.
        manager
            .alter_table(
                Table::alter()
                    .table(CrateIndexIden::Table)
                    .add_column(ColumnDef::new(CrateIndexIden::RustVersion).text())
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(CratesIoIndexIden::Table)
                    .add_column(ColumnDef::new(CratesIoIndexIden::RustVersion).text())
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(CratesIoIndexIden::Table)
                    .drop_column(CratesIoIndexIden::RustVersion)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(CrateIndexIden::Table)
                    .drop_column(CrateIndexIden::RustVersion)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
