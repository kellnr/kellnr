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
            Ok(resp) if resp.status().as_u16() < 300 => {
                tracing::debug!(
                    "Webhook callback sent successfully to: {}",
                    entry.callback_url
                );
                if let Err(err) = db.delete_webhook_queue(&entry.id).await {
                    tracing::error!("Cannot delete webhook queue entry. Reason {err}");
                }
            }
            Ok(resp) => {
                tracing::error!(
                    "Webhook callback failed for: {}. Response status: {}. Msg: {:?}",
                    entry.callback_url,
                    resp.status(),
                    resp.text().await
                );
                if let Err(err) = handle_failed_entry(db, &entry).await {
                    tracing::error!("Error while handling webhook failure: {err}");
                }
            }
            Err(err) => {
                tracing::error!(
                    "Webhook callback failed for: {}. Reason: {err}",
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
    match get_next_attempt(entry.last_attempt.as_ref(), &entry.next_attempt) {
        Some(next) => {
            db.update_webhook_queue(&entry.id, entry.next_attempt, next)
                .await?;
        }
        None => db.delete_webhook_queue(&entry.id).await?,
    }
    Ok(())
}

/// Resend timings according to:
/// <https://github.com/standard-webhooks/standard-webhooks/blob/main/spec/standard-webhooks.md#deliverability-and-reliability>
fn get_next_attempt(
    last_attempt: Option<&DateTime<Utc>>,
    current_attempt: &DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    let delta = last_attempt.map(|a| *current_attempt - a);
    println!("@@@@@ {delta:?}");

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

#[cfg(test)]
mod service_tests {
    use std::sync::Arc;

    use chrono::Utc;
    use common::{
        normalized_name::NormalizedName,
        version::Version,
        webhook::{Webhook, WebhookEvent},
    };
    use db::{ConString, Database, DbProvider, SqliteConString};

    use crate::{notify_crate, tests::get_test_listener};

    use super::*;

    #[tokio::test]
    async fn test_handle_queue_send_ok() {
        let db = get_db().await;

        for _ in 0..5 {
            let _ = db
                .register_webhook(sample_webhook(WebhookEvent::CrateAdd, 9980))
                .await
                .unwrap();
        }

        notify_crate(
            WebhookEvent::CrateAdd,
            &Utc::now(),
            &NormalizedName::from_unchecked_str("Test-Crate"),
            &Version::from_unchecked_str("0.1.0"),
            &db,
        )
        .await;

        let mut listener = get_test_listener(9980, 200).await;
        handle_queue(&db).await.unwrap();

        for _ in 0..5 {
            let listener_resp = listener.rx.recv().await.unwrap();
            assert_eq!(0, listener_resp);
        }

        let ts = Utc::now() + TimeDelta::minutes(5);
        let pending = db.get_pending_webhook_queue_entries(ts).await.unwrap();
        assert!(pending.is_empty());
    }

    #[tokio::test]
    async fn test_handle_queue_send_fail() {
        let db = get_db().await;

        let _ = db
            .register_webhook(sample_webhook(WebhookEvent::CrateAdd, 9981))
            .await
            .unwrap();

        notify_crate(
            WebhookEvent::CrateAdd,
            &Utc::now(),
            &NormalizedName::from_unchecked_str("Test-Crate"),
            &Version::from_unchecked_str("0.1.0"),
            &db,
        )
        .await;

        let mut listener = get_test_listener(9981, 400).await;
        handle_queue(&db).await.unwrap();

        let listener_resp = listener.rx.recv().await.unwrap();
        assert_eq!(0, listener_resp);

        let ts = Utc::now() + TimeDelta::minutes(5);
        let pending = db.get_pending_webhook_queue_entries(ts).await.unwrap();

        assert_eq!(pending.len(), 1);
        assert_eq!(
            pending[0].next_attempt,
            pending[0].last_attempt.unwrap() + TimeDelta::seconds(5)
        );

        // Try again to check the increasing interval
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        handle_queue(&db).await.unwrap();

        let ts = Utc::now() + TimeDelta::minutes(5);
        let pending = db.get_pending_webhook_queue_entries(ts).await.unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(
            pending[0].next_attempt,
            pending[0].last_attempt.unwrap() + TimeDelta::minutes(5)
        );
    }

    #[tokio::test]
    async fn test_handle_queue_send_no_response() {
        let db = get_db().await;

        let _ = db
            .register_webhook(sample_webhook(WebhookEvent::CrateAdd, 9982))
            .await
            .unwrap();

        notify_crate(
            WebhookEvent::CrateAdd,
            &Utc::now(),
            &NormalizedName::from_unchecked_str("Test-Crate"),
            &Version::from_unchecked_str("0.1.0"),
            &db,
        )
        .await;

        handle_queue(&db).await.unwrap();

        let ts = Utc::now() + TimeDelta::minutes(5);
        let pending = db.get_pending_webhook_queue_entries(ts).await.unwrap();
        assert_eq!(pending.len(), 1);
    }

    async fn get_db() -> Arc<dyn DbProvider> {
        let con_string = ConString::Sqlite(SqliteConString::new(
            std::path::Path::new(":memory:"),
            "salt",
            "admin",
            "token",
            std::time::Duration::from_secs(10),
        ));
        let db = Database::new(&con_string, 1).await.unwrap();
        Arc::new(db)
    }

    fn sample_webhook(event: WebhookEvent, callback_port: u16) -> Webhook {
        Webhook {
            id: None,
            event,
            callback_url: format!("http://0.0.0.0:{callback_port}"),
            name: None,
        }
    }
}
