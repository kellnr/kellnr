use sea_orm_migration::{prelude::*, schema::*};

use crate::iden::{CrateIndexIden, CrateUserIden, OwnerIden};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let owner_index = Index::create()
            .name("idx-owner")
            .table(OwnerIden::Table)
            .col(OwnerIden::CrateFk)
            .col(OwnerIden::UserFk)
            .unique()
            .to_owned();
        manager.create_index(owner_index).await?;

        let user_index = Index::create()
            .name("idx-crate-user")
            .table(CrateUserIden::Table)
            .col(CrateUserIden::CrateFk)
            .col(CrateUserIden::UserFk)
            .unique()
            .to_owned();
        manager.create_index(user_index).await?;

        let crate_index = Index::create()
            .name("idx-crate-index")
            .table(CrateIndexIden::Table)
            .col(CrateIndexIden::CrateFk)
            .col(CrateIndexIden::Vers)
            .unique()
            .to_owned();
        manager.create_index(crate_index).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let owner_index = Index::drop()
            .name("idx-owner")
            .table(OwnerIden::Table)
            .to_owned();
        manager.drop_index(owner_index).await?;

        let user_index = Index::drop()
            .name("idx-crate-user")
            .table(OwnerIden::Table)
            .to_owned();
        manager.drop_index(user_index).await?;

        let crate_index = Index::drop()
            .name("idx-crate-index")
            .table(CrateIndexIden::Table)
            .to_owned();
        manager.drop_index(crate_index).await
    }
}
