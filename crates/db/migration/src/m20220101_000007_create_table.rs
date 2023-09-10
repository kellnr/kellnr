use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Manual check if the column exists is needed, as Sqlite does not support
        // ALTER TABLE IF COLUMN EXISTS. Without the check, the migration would fail
        // on Sqlite with an "duplicate column" error.

        manager
            .create_table(
                Table::create()
                    .table(CratesIoIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CratesIoIden::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(CratesIoIden::Name)
                            .text()
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(CratesIoIden::OriginalName)
                            .text()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(CratesIoIden::ETag).string_len(64).not_null())
                    .col(ColumnDef::new(CratesIoIden::LastModified).text().not_null())
                    .col(ColumnDef::new(CratesIoIden::Description).text())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(CratesIoIndexIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CratesIoIndexIden::Id)
                            .big_integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(ColumnDef::new(CratesIoIndexIden::Name).text().not_null())
                    .col(ColumnDef::new(CratesIoIndexIden::Vers).text().not_null())
                    .col(ColumnDef::new(CratesIoIndexIden::Deps).json_binary())
                    .col(ColumnDef::new(CratesIoIndexIden::Cksum).text().not_null())
                    .col(ColumnDef::new(CratesIoIndexIden::Features).json_binary())
                    .col(ColumnDef::new(CratesIoIndexIden::Features2).json_binary())
                    .col(
                        ColumnDef::new(CratesIoIndexIden::Yanked)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(ColumnDef::new(CratesIoIndexIden::Links).text())
                    .col(
                        ColumnDef::new(CratesIoIndexIden::V)
                            .integer()
                            .not_null()
                            .default(1),
                    )
                    .col(
                        ColumnDef::new(CratesIoIndexIden::CratesIoFk)
                            .big_integer()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("cratesio_fk")
                            .from(CratesIoIndexIden::Table, CratesIoIndexIden::CratesIoFk)
                            .to(CratesIoIden::Table, CratesIoIden::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::NoAction),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop tables in reverse order of creation
        manager
            .drop_table(Table::drop().table(CratesIoIndexIden::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(CratesIoIden::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub enum CratesIoIden {
    #[iden = "cratesio_crate"]
    Table,
    Id,
    Name,
    OriginalName,
    Description,
    ETag,
    LastModified,
}

#[derive(Iden)]
pub enum CratesIoIndexIden {
    #[iden = "cratesio_index"]
    Table,
    Id,
    Name,
    Vers,
    Deps,
    Cksum,
    Features,
    Features2,
    Yanked,
    Links,
    V,
    CratesIoFk,
}
