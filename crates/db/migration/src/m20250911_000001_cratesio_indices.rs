use sea_orm_migration::prelude::*;
use sea_orm_migration::schema::*;

use crate::iden::{CratesIoIden, CratesIoIndexIden, CratesIoMetaIden};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .name("idx-crates-io-index-fk")
                    .table(CratesIoIndexIden::Table)
                    .col(CratesIoIndexIden::CratesIoFk)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-crates-io-meta")
                    .table(CratesIoMetaIden::Table)
                    .col(CratesIoMetaIden::CratesIoFk)
                    .col(CratesIoMetaIden::Version)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-crates-io-name")
                    .table(CratesIoIden::Table)
                    .col(CratesIoIden::Name)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx-crates-io-name")
                    .table(CratesIoIden::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx-crates-io-meta")
                    .table(CratesIoMetaIden::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx-crates-io-index-fk")
                    .table(CratesIoIndexIden::Table)
                    .to_owned(),
            )
            .await
    }
}
