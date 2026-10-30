pub use sea_orm_migration::prelude::*;
pub mod iden;
mod m20260126_000001_v6_baseline;
mod m20260126_000002_v6_upgrade;
mod m20260128_000001_oauth2_identity;
mod m20260129_000001_database_improvements;
mod m20260130_000001_toolchain;
mod m20260406_000001_toolchain_component;
mod m20260903_000001_index_rust_version;
mod m20261024_000001_webhook_secret;
mod m20261024_000002_hash_session_tokens;
mod m20261025_000001_auth_token_dates;
mod m20261025_000002_crate_meta_size;
mod m20261025_000003_crate_meta_publisher;
mod m20261027_000001_crate_dependency;
mod m20261028_000001_cratesio_meta_license;
mod m20261030_000001_cratesio_last_updated;
mod m20261030_000002_unique_crate_tags;
mod m20261031_000001_canonical_crate_names;

/// Fills the dependency table from the index, as the migration does. Public
/// for the database tests.
pub use m20261027_000001_crate_dependency::fill_dependencies as fill_crate_dependencies;
/// Sets the date of the cached crates.io crates, as the migration does.
/// Public for the database tests.
pub use m20261030_000001_cratesio_last_updated::fill_last_updated as fill_cratesio_last_updated;
/// Converts an HTTP Last-Modified header into the date format of the
/// database, as the migration does.
pub use m20261030_000001_cratesio_last_updated::http_date_to_db_date;
/// Removes duplicate links of crates to keywords, categories and authors, as
/// the migration does. Public for the database tests.
pub use m20261030_000002_unique_crate_tags::remove_duplicate_links as remove_duplicate_crate_tag_links;
/// Converts the crate names to their canonical form, as the migration does.
/// Public for the database tests.
pub use m20261031_000001_canonical_crate_names::canonicalize_crate_names;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260126_000001_v6_baseline::Migration),
            Box::new(m20260126_000002_v6_upgrade::Migration),
            Box::new(m20260128_000001_oauth2_identity::Migration),
            Box::new(m20260129_000001_database_improvements::Migration),
            Box::new(m20260130_000001_toolchain::Migration),
            Box::new(m20260406_000001_toolchain_component::Migration),
            Box::new(m20260903_000001_index_rust_version::Migration),
            Box::new(m20261024_000001_webhook_secret::Migration),
            Box::new(m20261024_000002_hash_session_tokens::Migration),
            Box::new(m20261025_000001_auth_token_dates::Migration),
            Box::new(m20261025_000002_crate_meta_size::Migration),
            Box::new(m20261025_000003_crate_meta_publisher::Migration),
            Box::new(m20261027_000001_crate_dependency::Migration),
            Box::new(m20261028_000001_cratesio_meta_license::Migration),
            Box::new(m20261030_000001_cratesio_last_updated::Migration),
            Box::new(m20261030_000002_unique_crate_tags::Migration),
            Box::new(m20261031_000001_canonical_crate_names::Migration),
        ]
    }
}
