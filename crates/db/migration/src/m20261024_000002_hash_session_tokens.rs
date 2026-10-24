use sea_orm_migration::prelude::*;

use crate::iden::SessionIden;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Session tokens are now stored as SHA-256 hashes. Existing rows hold
        // plaintext tokens that can no longer be looked up, so they are
        // removed instead of lingering until they expire. Users have to log
        // in once more after the upgrade.
        manager
            .exec_stmt(Query::delete().from_table(SessionIden::Table).to_owned())
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Hashed tokens cannot be turned back into plaintext ones.
        manager
            .exec_stmt(Query::delete().from_table(SessionIden::Table).to_owned())
            .await
    }
}
