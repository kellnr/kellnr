pub use sea_orm_migration::prelude::*;
pub mod iden;
mod m20260126_000001_v6_baseline;
mod m20260126_000002_v6_upgrade;
mod m20260128_000001_oauth2_identity;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260126_000001_v6_baseline::Migration),
            Box::new(m20260126_000002_v6_upgrade::Migration),
            Box::new(m20260128_000001_oauth2_identity::Migration),
        ]
    }
}
