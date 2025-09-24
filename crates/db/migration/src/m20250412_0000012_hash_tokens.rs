use crate::iden::AuthTokenIden;
use crate::sea_orm::ActiveValue::Set;
use crate::sea_orm::{ActiveModelTrait, EntityTrait};
use sea_orm::{ModelTrait, Related};
use sea_orm_migration::prelude::*;
use tracing::debug;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Make the "token" column in the auth_token table unique
        if !manager
            .has_index("auth_token", "idx_auth_token_token")
            .await?
        {
            debug!("Creating unique index on auth_token.token...");
            manager
                .create_index(
                    Index::create()
                        .table(AuthTokenIden::Table)
                        .name("idx_auth_token_token")
                        .col(AuthTokenIden::Token)
                        .unique()
                        .to_owned(),
                )
                .await?;
        }

        debug!("Hashing all tokens...");
        hash_all_tokens(manager.get_connection()).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Hash cannot be reversed...

        manager
            .drop_index(
                Index::drop()
                    .table(AuthTokenIden::Table)
                    .name("idx_auth_token_token")
                    .to_owned(),
            )
            .await
    }
}

async fn hash_all_tokens(db: &SchemaManagerConnection<'_>) -> Result<(), DbErr> {
    use crate::m20250412_0000012_hash_tokens_entities::auth_token;

    let tokens = auth_token::Entity::find()
        .all(db)
        .await?
        .into_iter()
        .collect::<Vec<_>>();

    for token in tokens {
        let hash = common::crypto::store_token(&token.token)
            .map_err(|err| DbErr::Migration(err.to_string()))?;
        let token_model = auth_token::ActiveModel {
            token: Set(hash),
            ..token.into()
        };
        token_model.update(db).await?;
    }

    Ok(())
}
