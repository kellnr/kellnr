use sea_orm_migration::prelude::*;

use crate::iden::WebhookIden;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Optional secret used to sign webhook deliveries. Existing webhooks
        // have no secret and keep receiving unsigned deliveries.
        manager
            .alter_table(
                Table::alter()
                    .table(WebhookIden::Table)
                    .add_column(ColumnDef::new(WebhookIden::Secret).text())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(WebhookIden::Table)
                    .drop_column(WebhookIden::Secret)
                    .to_owned(),
            )
            .await
    }
}
