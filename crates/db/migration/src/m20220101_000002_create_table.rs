use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Rename table "crate" to "krate" as "crate" is a keyword in rust
        manager
            .rename_table(
                sea_query::Table::rename()
                    .table(CrateIden::Table, Alias::new("krate"))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .rename_table(
                sea_query::Table::rename()
                    .table(Alias::new("krate"), CrateIden::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
pub enum CrateIden {
    #[iden = "crate"]
    Table,
}
