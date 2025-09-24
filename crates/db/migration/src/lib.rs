#![allow(dead_code)]

pub use sea_orm_migration::prelude::*;
pub mod iden;
mod m20220101_0000010_create_table;
mod m20220101_0000010_create_table_entities;
mod m20220101_0000011_create_table;
mod m20220101_0000011_create_table_entities;
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
mod m20250227_005754_add_readonly_user;
mod m20250227_005754_add_readonly_user_entities;
mod m20250319_191043_add_groups;
mod m20250319_191043_add_groups_entities;
mod m20250412_0000012_hash_tokens;
mod m20250412_0000012_hash_tokens_entities;
mod m20250414_102510_add_unique_indices;
mod m20250911_000001_cratesio_indices;
mod m20250923_095440_webhooks;
mod old_index_metadata;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_create_table::Migration),
            Box::new(m20220101_000001_create_table_entities::Migration),
            Box::new(m20220101_000002_create_table::Migration),
            Box::new(m20220101_000002_create_table_entities::Migration),
            Box::new(m20220101_000003_create_table::Migration),
            Box::new(m20220101_000003_create_table_entities::Migration),
            Box::new(m20220101_000004_create_table::Migration),
            Box::new(m20220101_000004_create_table_entities::Migration),
            Box::new(m20220101_000005_create_table::Migration),
            Box::new(m20220101_000005_create_table_entities::Migration),
            Box::new(m20220101_000006_create_table::Migration),
            Box::new(m20220101_000006_create_table_entities::Migration),
            Box::new(m20220101_000007_create_table::Migration),
            Box::new(m20220101_000007_create_table_entities::Migration),
            Box::new(m20220101_000008_create_table::Migration),
            Box::new(m20220101_000008_create_table_entities::Migration),
            Box::new(m20220101_000009_create_table::Migration),
            Box::new(m20220101_0000010_create_table::Migration),
            Box::new(m20220101_0000011_create_table::Migration),
            Box::new(m20220101_000009_create_table_entities::Migration),
            Box::new(m20250227_005754_add_readonly_user::Migration),
            Box::new(m20250227_005754_add_readonly_user_entities::Migration),
            Box::new(m20250319_191043_add_groups::Migration),
            Box::new(m20250319_191043_add_groups_entities::Migration),
            Box::new(m20250412_0000012_hash_tokens::Migration),
            Box::new(m20250414_102510_add_unique_indices::Migration),
            Box::new(m20250911_000001_cratesio_indices::Migration),
            Box::new(m20250923_095440_webhooks::Migration),
        ]
    }
}
