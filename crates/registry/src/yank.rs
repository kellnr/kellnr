use std::sync::Arc;

use chrono::Utc;
use kellnr_common::canonical_name::CanonicalName;
use kellnr_common::version::Version;
use kellnr_common::webhook::WebhookEvent;
use kellnr_db::DbProvider;
use kellnr_http::authz::{require_owner, require_writable};
use kellnr_http::error::ApiResult;
use kellnr_http::identity::Identity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YankAction {
    Yank,
    Unyank,
}

impl YankAction {
    fn webhook_event(self) -> WebhookEvent {
        match self {
            Self::Yank => WebhookEvent::CrateYank,
            Self::Unyank => WebhookEvent::CrateUnyank,
        }
    }
}

/// Yanks or unyanks `version` of `crate_name` for `user` and notifies the
/// webhooks. Only writable owners and admins may do so. The ownership check
/// comes first, so a non-owner gets a 403 for a missing crate as well and
/// cannot tell which crates exist. A missing version is a 404.
pub async fn change_yanked(
    action: YankAction,
    crate_name: &CanonicalName,
    version: &Version,
    user: &Identity,
    db: &Arc<dyn DbProvider>,
) -> ApiResult<()> {
    require_writable(user)?;
    require_owner(crate_name, user, db).await?;

    match action {
        YankAction::Yank => db.yank_crate(crate_name, version).await?,
        YankAction::Unyank => db.unyank_crate(crate_name, version).await?,
    }

    kellnr_webhooks::notify_crate(action.webhook_event(), &Utc::now(), crate_name, version, db)
        .await;

    Ok(())
}
