use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(GroupIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(GroupIden::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(GroupIden::Name)
                            .text()
                            .unique_key()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(GroupUserIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(GroupUserIden::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(GroupUserIden::GroupFk)
                            .big_integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("group_fk")
                            .from(GroupUserIden::Table, GroupUserIden::GroupFk)
                            .to(GroupIden::Table, GroupIden::Id)
                            .on_update(ForeignKeyAction::NoAction)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .col(
                        ColumnDef::new(GroupUserIden::UserFk)
                            .big_integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("user_fk")
                            .from(GroupUserIden::Table, GroupUserIden::UserFk)
                            .to(UserIden::Table, UserIden::Id)
                            .on_update(ForeignKeyAction::NoAction)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(CrateGroupIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CrateGroupIden::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(CrateGroupIden::CrateFk)
                            .big_integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("crate_fk")
                            .from(CrateGroupIden::Table, CrateGroupIden::CrateFk)
                            .to(CrateIden::Table, CrateIden::Id)
                            .on_update(ForeignKeyAction::NoAction)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .col(
                        ColumnDef::new(CrateGroupIden::GroupFk)
                            .big_integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("group_fk")
                            .from(CrateGroupIden::Table, CrateGroupIden::GroupFk)
                            .to(GroupIden::Table, GroupIden::Id)
                            .on_update(ForeignKeyAction::NoAction)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop tables in reverse order to their creation
        manager
            .drop_table(Table::drop().table(CrateGroupIden::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(GroupUserIden::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(GroupIden::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub enum GroupIden {
    #[iden = "group"]
    Table,
    Id,
    Name,
}

#[derive(Iden)]
pub enum GroupUserIden {
    #[iden = "group_user"]
    Table,
    Id,
    #[iden = "group_fk"]
    GroupFk,
    #[iden = "user_fk"]
    UserFk,
}

#[derive(Iden)]
pub enum UserIden {
    #[iden = "user"]
    Table,
    Id,
}

#[derive(Iden)]
pub enum CrateGroupIden {
    #[iden = "crate_group"]
    Table,
    Id,
    #[iden = "crate_fk"]
    CrateFk,
    #[iden = "group_fk"]
    GroupFk,
}

#[derive(Iden)]
pub enum CrateIden {
    #[iden = "krate"]
    Table,
    Id,
}
