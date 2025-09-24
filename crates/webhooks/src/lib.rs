use chrono::{Utc, DateTime};
use common::{normalized_name::NormalizedName, version::Version, webhook::WebhookAction};
use db::DbProvider;
use serde_json::json;

mod endpoints;
mod types;

pub use endpoints::{delete_webhook, get_all_webhooks, get_webhook, register_webhook};

pub async fn notify_crate(
    action: WebhookAction,
    timestamp: &DateTime<Utc>,
    normalized_name: &NormalizedName,
    version: &Version,
    db: &std::sync::Arc<dyn DbProvider>,
) {
    println!("{action:?}");
    println!("{timestamp:?}");
    println!("{normalized_name:?}");
    println!("{version:?}");

    let payload = json!({
        "type": action,
        "timestamp": timestamp,
        "data": {
            "crate_name": format!("{}", normalized_name),
            "crate_version": version
        }
    });
    println!("Payload: {payload:?}");

    if let Err(err) = db.add_webhook_queue(action, payload).await {
        tracing::error!("Db: {err:?}");
    }
}
