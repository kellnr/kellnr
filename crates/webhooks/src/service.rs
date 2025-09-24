use chrono::{DateTime, TimeDelta, Utc};
use common::webhook::WebhookQueue;
use db::DbProvider;
use std::sync::Arc;

use crate::types::WebhookError;

pub fn run_webhook_service(db: Arc<dyn DbProvider>) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            if let Err(err) = handle_queue(&db).await {
                tracing::error!("Webhook queue failed. Reason {err}");
            }
        }
    });
}

async fn handle_queue(db: &Arc<dyn DbProvider>) -> Result<(), WebhookError> {
    let now = Utc::now();
    let pending = db.get_pending_webhook_queue_entries(now).await?;

    let client = reqwest::Client::new();

    for entry in pending {
        let request = client.post(&entry.callback_url).json(&entry.payload);

        match request.send().await {
            Ok(_) => {
                tracing::debug!(
                    "Webhook callback sent successfully to: {}",
                    entry.callback_url
                );
                if let Err(err) = db.delete_webhook_queue(&entry.id).await {
                    tracing::error!("Cannot delete webhook queue entry. Reason {err}");
                }
            }
            Err(err) => {
                tracing::error!(
                    "Webhook callback failed at: {}. Reason: {err}",
                    entry.callback_url
                );
                if let Err(err) = handle_failed_entry(db, &entry).await {
                    tracing::error!("Error while handling webhook failure: {err}");
                }
            }
        }
    }
    Ok(())
}

async fn handle_failed_entry(
    db: &Arc<dyn DbProvider>,
    entry: &WebhookQueue,
) -> Result<(), WebhookError> {
    match get_next_attempt(&entry.last_attempt, &entry.next_attempt) {
        Some(next) => {
            db.update_webhook_queue(&entry.id, entry.next_attempt, next)
                .await?
        }
        None => db.delete_webhook_queue(&entry.id).await?,
    }
    Ok(())
}

/// Resend timings according to:
/// https://github.com/standard-webhooks/standard-webhooks/blob/main/spec/standard-webhooks.md#deliverability-and-reliability
fn get_next_attempt(
    last_attempt: &Option<DateTime<Utc>>,
    current_attempt: &DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    let delta = last_attempt.map(|a| *current_attempt - a);

    let offset = match delta {
        // First try
        None => TimeDelta::seconds(5),
        Some(d) if d < TimeDelta::minutes(5) => TimeDelta::minutes(5),
        Some(d) if d < TimeDelta::minutes(30) => TimeDelta::minutes(30),
        Some(d) if d < TimeDelta::hours(2) => TimeDelta::hours(2),
        Some(d) if d < TimeDelta::hours(5) => TimeDelta::hours(5),
        Some(d) if d < TimeDelta::hours(10) => TimeDelta::hours(10),
        Some(d) if d < TimeDelta::hours(14) => TimeDelta::hours(14),
        Some(d) if d < TimeDelta::hours(20) => TimeDelta::hours(20),
        Some(d) if d < TimeDelta::hours(24) => TimeDelta::hours(24),
        // More than 24 hours -> do not send anymore.
        _ => return None,
    };
    Some(*current_attempt + offset)
}
