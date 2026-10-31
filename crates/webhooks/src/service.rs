use std::sync::Arc;

use chrono::{DateTime, TimeDelta, Utc};
use futures_util::{StreamExt, stream};
use kellnr_common::webhook::WebhookQueue;
use kellnr_db::DbProvider;
use kellnr_settings::Webhooks;

use crate::client::{Delivery, WebhookClient, truncated_body};
use crate::types::WebhookError;

const MAX_CONCURRENT_DELIVERIES: usize = 8;

pub fn run_webhook_service(db: Arc<dyn DbProvider>, settings: &Webhooks) {
    let proxy_in_env = ["HTTP_PROXY", "http_proxy", "HTTPS_PROXY", "https_proxy"]
        .iter()
        .any(|var| std::env::var_os(var).is_some());
    if settings.block_private_targets && !settings.ignore_env_proxy && proxy_in_env {
        tracing::warn!(
            "Webhooks are sent through the proxy from the environment, so only the proxy's \
             address is checked against the blocked targets. Set webhooks.ignore_env_proxy \
             to connect directly."
        );
    }
    let http_client = match WebhookClient::new(settings) {
        Ok(client) => client,
        Err(err) => {
            tracing::error!("Cannot create webhook HTTP client, webhooks are disabled: {err}");
            return;
        }
    };
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            if let Err(err) = handle_queue(&db, &http_client).await {
                tracing::error!("Webhook queue failed. Reason {err}");
            }
        }
    });
}

async fn handle_queue(
    db: &Arc<dyn DbProvider>,
    client: &WebhookClient,
) -> Result<(), WebhookError> {
    let now = Utc::now();
    let pending = db.get_pending_webhook_queue_entries(now).await?;

    // Deliver concurrently, so a few slow callback hosts cannot delay all
    // other deliveries by up to the request timeout each.
    stream::iter(pending)
        .for_each_concurrent(MAX_CONCURRENT_DELIVERIES, |entry| {
            deliver(db, client, entry)
        })
        .await;
    Ok(())
}

async fn deliver(db: &Arc<dyn DbProvider>, client: &WebhookClient, entry: WebhookQueue) {
    let delivery = Delivery {
        id: &entry.id,
        payload: &entry.payload,
        secret: entry.secret.as_deref(),
    };
    match client.post(&entry.callback_url, &delivery).await {
        Ok(resp) if resp.status().as_u16() < 300 => {
            if let Err(err) = db.delete_webhook_queue(&entry.id).await {
                tracing::error!("Cannot delete webhook queue entry. Reason {err}");
            }
        }
        Ok(resp) => {
            tracing::error!(
                "Webhook callback failed for: {}. Response status: {}. Msg: {:?}",
                entry.callback_url,
                resp.status(),
                truncated_body(resp).await
            );
            if let Err(err) = handle_failed_entry(db, &entry).await {
                tracing::error!("Error while handling webhook failure: {err}");
            }
        }
        Err(err) if err.is_permanent() => {
            tracing::error!(
                "Webhook callback failed for: {}. Reason: {err}. Not retrying, the error is permanent",
                entry.callback_url
            );
            if let Err(err) = db.delete_webhook_queue(&entry.id).await {
                tracing::error!("Cannot delete webhook queue entry. Reason {err}");
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
///
/// The delays between the attempts are 5s, 5m, 30m, 2h, 5h, 10h, 14h, 20h
/// and 24h, so the last attempt is about 76 hours after the first one.
fn get_next_attempt(
    last_attempt: Option<&DateTime<Utc>>,
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
        // The last attempt was 24 hours after the previous one.
        _ => return None,
    };
    Some(*current_attempt + offset)
}

#[cfg(test)]
mod service_tests {
    use std::sync::Arc;

    use chrono::Utc;
    use kellnr_common::canonical_name::CanonicalName;
    use kellnr_common::publish_metadata::PublishMetadata;
    use kellnr_common::version::Version;
    use kellnr_common::webhook::{Webhook, WebhookEvent};
    use kellnr_db::{ConString, Database, DbProvider, SqliteConString, UserFlags};

    use super::*;
    use crate::notify_crate;
    use crate::tests::get_test_listener;

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
            &CanonicalName::try_from("Test-Crate").unwrap(),
            &Version::try_from("0.1.0").unwrap(),
            &db,
        )
        .await;

        let mut listener = get_test_listener(9980, 200).await;
        let http_client = WebhookClient::new(&Webhooks::default()).unwrap();
        handle_queue(&db, &http_client).await.unwrap();

        for _ in 0..5 {
            let listener_resp = listener.rx.recv().await.unwrap();
            assert_eq!(0, listener_resp);
        }

        let ts = Utc::now() + TimeDelta::minutes(5);
        let pending = db.get_pending_webhook_queue_entries(ts).await.unwrap();
        assert!(pending.is_empty());
    }

    #[tokio::test]
    async fn test_handle_queue_delivers_concurrently() {
        let db = get_db().await;
        let deliveries = 8;
        let delay = std::time::Duration::from_secs(1);

        for _ in 0..deliveries {
            db.register_webhook(sample_webhook(WebhookEvent::CrateAdd, 9989))
                .await
                .unwrap();
        }
        notify_crate(
            WebhookEvent::CrateAdd,
            &Utc::now(),
            &CanonicalName::try_from("Test-Crate").unwrap(),
            &Version::try_from("0.1.0").unwrap(),
            &db,
        )
        .await;

        let listener = spawn_slow_listener(9989, delay).await;
        let http_client = WebhookClient::new(&Webhooks::default()).unwrap();
        let start = std::time::Instant::now();
        handle_queue(&db, &http_client).await.unwrap();
        let elapsed = start.elapsed();
        listener.abort();

        assert!(elapsed < delay * 4, "deliveries took {elapsed:?}");
        let ts = Utc::now() + TimeDelta::minutes(5);
        assert!(
            db.get_pending_webhook_queue_entries(ts)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn test_handle_queue_records_mixed_results() {
        let db = get_db().await;

        for port in [9990, 9990, 9990, 9991, 9991, 9991, 9992] {
            db.register_webhook(sample_webhook(WebhookEvent::CrateAdd, port))
                .await
                .unwrap();
        }
        notify_crate(
            WebhookEvent::CrateAdd,
            &Utc::now(),
            &CanonicalName::try_from("Test-Crate").unwrap(),
            &Version::try_from("0.1.0").unwrap(),
            &db,
        )
        .await;

        let _ok = get_test_listener(9990, 200).await;
        let _fail = get_test_listener(9991, 500).await;
        // Nothing listens on port 9992.
        let http_client = WebhookClient::new(&Webhooks::default()).unwrap();
        handle_queue(&db, &http_client).await.unwrap();

        let ts = Utc::now() + TimeDelta::minutes(5);
        let pending = db.get_pending_webhook_queue_entries(ts).await.unwrap();
        assert_eq!(4, pending.len());
        for entry in pending {
            assert_eq!(
                entry.next_attempt,
                entry.last_attempt.unwrap() + TimeDelta::seconds(5)
            );
        }
    }

    /// Listener that answers every connection with 200 after `delay`,
    /// handling connections in parallel.
    async fn spawn_slow_listener(
        port: u16,
        delay: std::time::Duration,
    ) -> tokio::task::JoinHandle<()> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
            .await
            .unwrap();
        tokio::spawn(async move {
            loop {
                let (mut stream, _) = listener.accept().await.unwrap();
                tokio::spawn(async move {
                    let mut buf = [0; 4096];
                    let _ = stream.read(&mut buf).await;
                    tokio::time::sleep(delay).await;
                    let _ = stream
                        .write_all(b"HTTP/1.1 200\r\nContent-Length: 0\r\n\r\n")
                        .await;
                });
            }
        })
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
            &CanonicalName::try_from("Test-Crate").unwrap(),
            &Version::try_from("0.1.0").unwrap(),
            &db,
        )
        .await;

        let mut listener = get_test_listener(9981, 400).await;
        let http_client = WebhookClient::new(&Webhooks::default()).unwrap();
        handle_queue(&db, &http_client).await.unwrap();

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
        handle_queue(&db, &http_client).await.unwrap();

        let ts = Utc::now() + TimeDelta::minutes(5);
        let pending = db.get_pending_webhook_queue_entries(ts).await.unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(
            pending[0].next_attempt,
            pending[0].last_attempt.unwrap() + TimeDelta::minutes(5)
        );
    }

    #[tokio::test]
    async fn test_handle_queue_drops_entries_with_permanent_errors() {
        let db = get_db().await;
        for callback_url in ["http://169.254.169.254/", "not a url"] {
            db.register_webhook(Webhook {
                callback_url: callback_url.to_string(),
                ..sample_webhook(WebhookEvent::CrateAdd, 0)
            })
            .await
            .unwrap();
        }
        notify_crate(
            WebhookEvent::CrateAdd,
            &Utc::now(),
            &CanonicalName::try_from("Test-Crate").unwrap(),
            &Version::try_from("0.1.0").unwrap(),
            &db,
        )
        .await;

        let http_client = WebhookClient::new(&Webhooks::default()).unwrap();
        handle_queue(&db, &http_client).await.unwrap();

        let ts = Utc::now() + TimeDelta::days(30);
        assert!(
            db.get_pending_webhook_queue_entries(ts)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn notify_crate_adds_original_name_to_payload() {
        let db = get_db().await;
        db.register_webhook(sample_webhook(WebhookEvent::CrateYank, 9995))
            .await
            .unwrap();
        db.add_user("owner", "pwd", UserFlags::normal())
            .await
            .unwrap();
        db.add_crate(
            &PublishMetadata::minimal("Test-Crate", "0.1.0"),
            "cksum",
            1,
            &Utc::now(),
            "owner",
            None,
        )
        .await
        .unwrap();

        notify_crate(
            WebhookEvent::CrateYank,
            &Utc::now(),
            &CanonicalName::try_from("test-crate").unwrap(),
            &Version::try_from("0.1.0").unwrap(),
            &db,
        )
        .await;

        let pending = db
            .get_pending_webhook_queue_entries(Utc::now() + TimeDelta::minutes(5))
            .await
            .unwrap();
        assert_eq!(pending[0].payload["data"]["crate_name"], "test-crate");
        assert_eq!(pending[0].payload["data"]["original_name"], "Test-Crate");
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
            &CanonicalName::try_from("Test-Crate").unwrap(),
            &Version::try_from("0.1.0").unwrap(),
            &db,
        )
        .await;

        let http_client = WebhookClient::new(&Webhooks::default()).unwrap();
        handle_queue(&db, &http_client).await.unwrap();

        let ts = Utc::now() + TimeDelta::minutes(5);
        let pending = db.get_pending_webhook_queue_entries(ts).await.unwrap();
        assert_eq!(pending.len(), 1);
    }

    async fn get_db() -> Arc<dyn DbProvider> {
        let con_string = ConString::Sqlite(SqliteConString::new(
            std::path::Path::new(":memory:"),
            "admin",
            Some("token".to_string()),
            kellnr_settings::SessionAge::from_secs(10),
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
            secret: None,
        }
    }
}
