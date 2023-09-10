use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(UserIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(UserIden::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(UserIden::Name)
                            .text()
                            .unique_key()
                            .not_null(),
                    )
                    .col(ColumnDef::new(UserIden::Pwd).text().not_null())
                    .col(ColumnDef::new(UserIden::Salt).text().not_null())
                    .col(ColumnDef::new(UserIden::IsAdmin).boolean().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(SessionIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SessionIden::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(SessionIden::Token)
                            .text()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(SessionIden::Created).text().not_null())
                    .col(ColumnDef::new(SessionIden::UserFk).big_integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("user_fk")
                            .from(SessionIden::Table, SessionIden::UserFk)
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
                    .table(CrateIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CrateIden::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(CrateIden::Name)
                            .text()
                            .not_null()
                            .unique_key(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(CrateMetaIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CrateMetaIden::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(ColumnDef::new(CrateMetaIden::Version).text().not_null())
                    .col(ColumnDef::new(CrateMetaIden::Created).text().not_null())
                    .col(
                        ColumnDef::new(CrateMetaIden::Downloads)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .to_owned()
                    .col(
                        ColumnDef::new(CrateMetaIden::CrateFk)
                            .big_integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("crate_fk")
                            .from(CrateMetaIden::Table, CrateMetaIden::CrateFk)
                            .to(CrateIden::Table, CrateIden::Id)
                            .on_update(ForeignKeyAction::NoAction)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(OwnerIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(OwnerIden::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(ColumnDef::new(OwnerIden::CrateFk).big_integer().not_null())
                    .col(ColumnDef::new(OwnerIden::UserFk).big_integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("crate_fk")
                            .from(OwnerIden::Table, OwnerIden::CrateFk)
                            .to(CrateIden::Table, CrateIden::Id)
                            .on_update(ForeignKeyAction::NoAction)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("user_fk")
                            .from(OwnerIden::Table, OwnerIden::UserFk)
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
                    .table(AuthTokenIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AuthTokenIden::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(ColumnDef::new(AuthTokenIden::Name).text().not_null())
                    .col(ColumnDef::new(AuthTokenIden::Token).text().not_null())
                    .col(
                        ColumnDef::new(AuthTokenIden::UserFk)
                            .big_integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("user_fk")
                            .from(AuthTokenIden::Table, AuthTokenIden::UserFk)
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
                    .table(DocQueueIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(DocQueueIden::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(ColumnDef::new(DocQueueIden::Krate).text().not_null())
                    .col(ColumnDef::new(DocQueueIden::Version).text().not_null())
                    .col(ColumnDef::new(DocQueueIden::Path).text().not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop tables in reverse order to their creation
        manager
            .drop_table(Table::drop().table(DocQueueIden::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(AuthTokenIden::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(OwnerIden::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(CrateMetaIden::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(CrateIden::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(SessionIden::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(UserIden::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub enum UserIden {
    #[iden = "user"]
    Table,
    Id,
    Name,
    Pwd,
    Salt,
    IsAdmin,
}

#[derive(Iden)]
pub enum SessionIden {
    #[iden = "session"]
    Table,
    Id,
    Token,
    Created,
    #[iden = "user_fk"]
    UserFk,
}

#[derive(Iden)]
pub enum CrateIden {
    #[iden = "crate"]
    Table,
    Id,
    Name,
}

#[derive(Iden)]
pub enum CrateMetaIden {
    #[iden = "crate_meta"]
    Table,
    Id,
    Version,
    Created,
    Downloads,
    #[iden = "crate_fk"]
    CrateFk,
}

#[derive(Iden)]
pub enum OwnerIden {
    #[iden = "owner"]
    Table,
    Id,
    #[iden = "crate_fk"]
    CrateFk,
    #[iden = "user_fk"]
    UserFk,
}

#[derive(Iden)]
pub enum AuthTokenIden {
    #[iden = "auth_token"]
    Table,
    Id,
    Name,
    Token,
    #[iden = "user_fk"]
    UserFk,
}

#[derive(Iden)]
pub enum DocQueueIden {
    #[iden = "doc_queue"]
    Table,
    Id,
    Krate,
    Version,
    Path,
}
