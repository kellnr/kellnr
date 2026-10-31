use chrono::{DateTime, Utc};
use kellnr_common::canonical_name::CanonicalName;
use kellnr_common::version::Version;
use kellnr_common::webhook::WebhookEvent;
use kellnr_db::DbProvider;
use serde_json::json;

mod client;
pub mod endpoints;
mod service;
mod signature;
mod target;
#[cfg(test)]
mod tests;
mod types;

pub use endpoints::{
    delete_webhook, get_all_webhooks, get_webhook, register_webhook, test_webhook,
};
pub use service::run_webhook_service;

/// Queue a webhook message for all webhooks of `event`. The payload contains
/// the lowercase crate name as `crate_name` and the name the crate was
/// published with as `original_name`.
pub async fn notify_crate(
    event: WebhookEvent,
    timestamp: &DateTime<Utc>,
    crate_name: &CanonicalName,
    version: &Version,
    db: &std::sync::Arc<dyn DbProvider>,
) {
    let original_name = match db.get_original_name(crate_name, version).await {
        Ok(Some(original_name)) => original_name.to_string(),
        Ok(None) => crate_name.to_string(),
        Err(err) => {
            tracing::error!("Cannot read the original name of {crate_name}: {err}");
            crate_name.to_string()
        }
    };
    let payload = json!({
        "type": event,
        "timestamp": timestamp,
        "data": {
            "crate_name": original_name.to_lowercase(),
            "original_name": original_name,
            "crate_version": version
        }
    });

    if let Err(err) = db.add_webhook_queue(event, payload).await {
        tracing::error!("Db: {err:?}");
    }
}
