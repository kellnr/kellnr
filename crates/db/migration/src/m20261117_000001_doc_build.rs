use sea_orm_migration::prelude::*;

use crate::iden::{CrateMetaIden, DocBuildIden};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // The results of finished rustdoc builds, shown on the doc queue page.
        // Removed together with their version.
        manager
            .create_table(
                Table::create()
                    .table(DocBuildIden::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(DocBuildIden::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(DocBuildIden::CrateMetaFk)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(DocBuildIden::Status).text().not_null())
                    .col(ColumnDef::new(DocBuildIden::Error).text())
                    .col(ColumnDef::new(DocBuildIden::Started).text().not_null())
                    .col(ColumnDef::new(DocBuildIden::Finished).text().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("doc_build_crate_meta_fk")
                            .from(DocBuildIden::Table, DocBuildIden::CrateMetaFk)
                            .to(CrateMetaIden::Table, CrateMetaIden::Id)
                            .on_update(ForeignKeyAction::NoAction)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Deleting a version looks up its builds by the foreign key
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_doc_build_crate_meta_fk")
                    .table(DocBuildIden::Table)
                    .col(DocBuildIden::CrateMetaFk)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(DocBuildIden::Table).to_owned())
            .await
    }
}
