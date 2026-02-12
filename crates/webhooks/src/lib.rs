use chrono::{DateTime, Utc};
use kellnr_common::normalized_name::NormalizedName;
use kellnr_common::version::Version;
use kellnr_common::webhook::WebhookEvent;
use kellnr_db::DbProvider;
use serde_json::json;

pub mod endpoints;
mod service;
#[cfg(test)]
mod tests;
pub mod types;

pub use endpoints::{
    delete_webhook, get_all_webhooks, get_webhook, register_webhook, test_webhook,
};
pub use service::run_webhook_service;

pub async fn notify_crate(
    event: WebhookEvent,
    timestamp: &DateTime<Utc>,
    normalized_name: &NormalizedName,
    version: &Version,
    db: &std::sync::Arc<dyn DbProvider>,
) {
    let payload = json!({
        "type": event,
        "timestamp": timestamp,
        "data": {
            "crate_name": format!("{}", normalized_name),
            "crate_version": version
        }
    });

    if let Err(err) = db.add_webhook_queue(event, payload).await {
        tracing::error!("Db: {err:?}");
    }
}
