use sea_orm_migration::{prelude::*, schema::*};

use crate::iden::{
    CrateGroupIden, CrateIndexIden, CrateMetaIden, CrateUserIden, GroupUserIden, OwnerIden,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .name("idx-owner")
                    .table(OwnerIden::Table)
                    .col(OwnerIden::CrateFk)
                    .col(OwnerIden::UserFk)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-crate-user")
                    .table(CrateUserIden::Table)
                    .col(CrateUserIden::CrateFk)
                    .col(CrateUserIden::UserFk)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-crate-index")
                    .table(CrateIndexIden::Table)
                    .col(CrateIndexIden::CrateFk)
                    .col(CrateIndexIden::Vers)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-crate-meta")
                    .table(CrateMetaIden::Table)
                    .col(CrateMetaIden::CrateFk)
                    .col(CrateMetaIden::Version)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-group-user")
                    .table(GroupUserIden::Table)
                    .col(GroupUserIden::UserFk)
                    .col(GroupUserIden::GroupFk)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-crate-group")
                    .table(CrateGroupIden::Table)
                    .col(CrateGroupIden::CrateFk)
                    .col(CrateGroupIden::GroupFk)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx-owner")
                    .table(OwnerIden::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx-crate-user")
                    .table(CrateUserIden::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx-crate-index")
                    .table(CrateIndexIden::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx-crate-meta")
                    .table(CrateMetaIden::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx-group-user")
                    .table(GroupUserIden::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx-crate-group")
                    .table(CrateGroupIden::Table)
                    .to_owned(),
            )
            .await
    }
}
