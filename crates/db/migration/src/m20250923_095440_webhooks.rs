use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(WebhookIden::Table)
                    .if_not_exists()
                    .col(pk_uuid(WebhookIden::Id))
                    .col(string(WebhookIden::Action))
                    .col(string(WebhookIden::CallbackUrl))
                    .col(string_null(WebhookIden::Name))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(WebhookQueueIden::Table)
                    .if_not_exists()
                    .col(pk_uuid(WebhookQueueIden::Id))
                    .col(uuid(WebhookQueueIden::WebhookFk))
                    .col(json(WebhookQueueIden::Payload))
                    .col(date_time_null(WebhookQueueIden::LastAttempt))
                    .col(date_time(WebhookQueueIden::NextAttempt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("webhook_fk")
                            .from(WebhookQueueIden::Table, WebhookQueueIden::WebhookFk)
                            .to(WebhookIden::Table, WebhookIden::Id)
                            .on_update(ForeignKeyAction::NoAction)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(WebhookIden::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(WebhookQueueIden::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum WebhookIden {
    #[iden = "webhook"]
    Table,
    Id,
    Action,
    #[iden = "callback_url"]
    CallbackUrl,
    Name
}

#[derive(Iden)]
enum WebhookQueueIden {
    #[iden = "webhook_queue"]
    Table,
    Id,
    #[iden = "webhook_fk"]
    WebhookFk,
    Payload,
    #[iden = "last_attempt"]
    LastAttempt,
    #[iden = "next_attempt"]
    NextAttempt
}
