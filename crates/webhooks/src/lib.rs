use chrono::{DateTime, Utc};
use common::{normalized_name::NormalizedName, version::Version, webhook::WebhookAction};
use db::DbProvider;
use serde_json::json;

mod endpoints;
mod service;
#[cfg(test)]
mod tests;
mod types;

pub use endpoints::{
    delete_webhook, get_all_webhooks, get_webhook, register_webhook, test_webhook,
};
pub use service::run_webhook_service;

pub async fn notify_crate(
    action: WebhookAction,
    timestamp: &DateTime<Utc>,
    normalized_name: &NormalizedName,
    version: &Version,
    db: &std::sync::Arc<dyn DbProvider>,
) {
    let payload = json!({
        "type": action,
        "timestamp": timestamp,
        "data": {
            "crate_name": format!("{}", normalized_name),
            "crate_version": version
        }
    });

    if let Err(err) = db.add_webhook_queue(action, payload).await {
        tracing::error!("Db: {err:?}");
    }
}
