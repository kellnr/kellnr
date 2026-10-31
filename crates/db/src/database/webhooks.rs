use chrono::{DateTime, Utc};
use kellnr_common::webhook::{Webhook, WebhookEvent, WebhookQueue};
use kellnr_entity::{webhook, webhook_queue};
use sea_orm::entity::prelude::Uuid;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, ModelTrait, QueryFilter, Set};
use tracing::warn;

use super::Database;
use crate::error::DbError;
use crate::provider::DbResult;

fn webhook_model_to_obj(w: webhook::Model) -> DbResult<Webhook> {
    let event = w
        .event
        .as_str()
        .try_into()
        .map_err(|_| DbError::InvalidWebhookEvent(w.event))?;
    Ok(Webhook {
        id: Some(w.id.into()),
        name: w.name,
        event,
        callback_url: w.callback_url,
        secret: w.secret,
    })
}

impl Database {
    async fn get_webhook_model(&self, id: &str) -> DbResult<webhook::Model> {
        webhook::Entity::find()
            .filter(webhook::Column::Id.eq(
                TryInto::<Uuid>::try_into(id).map_err(|_| DbError::InvalidId(id.to_string()))?,
            ))
            .one(&self.db_con)
            .await?
            .ok_or(DbError::WebhookNotFound)
    }

    async fn get_webhook_queue_model(&self, id: &str) -> DbResult<webhook_queue::Model> {
        webhook_queue::Entity::find()
            .filter(webhook_queue::Column::Id.eq(
                TryInto::<Uuid>::try_into(id).map_err(|_| DbError::InvalidId(id.to_string()))?,
            ))
            .one(&self.db_con)
            .await?
            .ok_or(DbError::WebhookNotFound)
    }

    pub(super) async fn register_webhook(&self, webhook: Webhook) -> DbResult<String> {
        let w = webhook::ActiveModel {
            event: Set(Into::<&str>::into(webhook.event).to_string()),
            callback_url: Set(webhook.callback_url),
            name: Set(webhook.name),
            secret: Set(webhook.secret),
            ..Default::default()
        };

        let w: webhook::Model = w.insert(&self.db_con).await?;
        Ok(w.id.to_string())
    }

    pub(super) async fn delete_webhook(&self, id: &str) -> DbResult<()> {
        self.get_webhook_model(id)
            .await?
            .delete(&self.db_con)
            .await?;
        Ok(())
    }

    pub(super) async fn get_webhook(&self, id: &str) -> DbResult<Webhook> {
        webhook_model_to_obj(self.get_webhook_model(id).await?)
    }

    pub(super) async fn get_all_webhooks(&self) -> DbResult<Vec<Webhook>> {
        Ok(webhook::Entity::find()
            .all(&self.db_con)
            .await?
            .into_iter()
            .filter_map(|w| {
                let id = w.id;
                webhook_model_to_obj(w)
                    .inspect_err(|e| warn!("Skipping invalid webhook {id}: {e}"))
                    .ok()
            })
            .collect())
    }

    pub(super) async fn add_webhook_queue(
        &self,
        event: WebhookEvent,
        payload: serde_json::Value,
    ) -> DbResult<()> {
        let w = webhook::Entity::find()
            .filter(webhook::Column::Event.eq(Into::<&str>::into(event)))
            .all(&self.db_con)
            .await?;

        if w.is_empty() {
            return Ok(());
        }

        let now = Utc::now();

        let entries = w.into_iter().map(|w| webhook_queue::ActiveModel {
            webhook_fk: Set(w.id),
            payload: Set(payload.clone()),
            next_attempt: Set(now.into()),
            last_attempt: Set(None),
            ..Default::default()
        });

        webhook_queue::Entity::insert_many(entries)
            .exec(&self.db_con)
            .await?;
        Ok(())
    }

    pub(super) async fn get_pending_webhook_queue_entries(
        &self,
        timestamp: DateTime<Utc>,
    ) -> DbResult<Vec<WebhookQueue>> {
        let w = webhook_queue::Entity::find()
            .find_with_related(webhook::Entity)
            .filter(webhook_queue::Column::NextAttempt.lte(timestamp))
            .all(&self.db_con)
            .await?;

        Ok(w.into_iter()
            .filter_map(|w| {
                let webhook = w.1.first()?;
                Some(WebhookQueue {
                    id: w.0.id.to_string(),
                    callback_url: webhook.callback_url.clone(),
                    payload: w.0.payload,
                    last_attempt: w.0.last_attempt.map(Into::into),
                    next_attempt: w.0.next_attempt.into(),
                    secret: webhook.secret.clone(),
                })
            })
            .collect())
    }

    pub(super) async fn update_webhook_queue(
        &self,
        id: &str,
        last_attempt: DateTime<Utc>,
        next_attempt: DateTime<Utc>,
    ) -> DbResult<()> {
        let mut w: webhook_queue::ActiveModel = self.get_webhook_queue_model(id).await?.into();
        w.last_attempt = Set(Some(last_attempt.into()));
        w.next_attempt = Set(next_attempt.into());
        w.update(&self.db_con).await?;
        Ok(())
    }

    pub(super) async fn delete_webhook_queue(&self, id: &str) -> DbResult<()> {
        self.get_webhook_queue_model(id)
            .await?
            .delete(&self.db_con)
            .await?;
        Ok(())
    }
}
