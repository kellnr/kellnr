pub use sea_orm_migration::prelude::*;
pub mod iden;
mod m20220101_000001_create_table;
mod m20220101_000001_create_table_entities;
mod m20220101_000002_create_table;
mod m20220101_000002_create_table_entities;
mod m20220101_000003_create_table;
mod m20220101_000003_create_table_entities;
mod m20220101_000004_create_table;
mod m20220101_000004_create_table_entities;
mod m20220101_000005_create_table;
mod m20220101_000005_create_table_entities;
mod m20220101_000006_create_table;
mod m20220101_000006_create_table_entities;
mod m20220101_000007_create_table;
mod m20220101_000007_create_table_entities;
mod m20220101_000008_create_table;
mod m20220101_000008_create_table_entities;
mod m20220101_000009_create_table;
mod m20220101_000009_create_table_entities;
mod old_index_metadata;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_create_table::Migration),
            Box::new(m20220101_000002_create_table::Migration),
            Box::new(m20220101_000003_create_table::Migration),
            Box::new(m20220101_000004_create_table::Migration),
            Box::new(m20220101_000005_create_table::Migration),
            Box::new(m20220101_000006_create_table::Migration),
            Box::new(m20220101_000007_create_table::Migration),
            Box::new(m20220101_000008_create_table::Migration),
            Box::new(m20220101_000009_create_table::Migration),
        ]
    }
}
