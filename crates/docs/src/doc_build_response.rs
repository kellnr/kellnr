use chrono::{DateTime, Utc};
use kellnr_db::{DocBuildStatus, RecordedDocBuild};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Recently finished documentation builds
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, ToSchema)]
pub struct DocBuildsResponse {
    /// Finished builds, newest first
    pub(crate) builds: Vec<DocBuildResponse>,
}

/// A finished documentation build
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, ToSchema)]
pub(crate) struct DocBuildResponse {
    /// Id of the build
    pub(crate) id: i64,
    /// Crate name
    pub(crate) name: String,
    /// Crate version
    pub(crate) version: String,
    pub(crate) status: DocBuildStatus,
    pub(crate) started: DateTime<Utc>,
    pub(crate) finished: DateTime<Utc>,
    /// Why the build failed. Only shown to admins and owners of the crate.
    pub(crate) error: Option<String>,
    /// Whether the user may queue the build again
    pub(crate) can_rebuild: bool,
}

/// What a user may see of the builds of a crate.
#[derive(Clone, Copy)]
pub(crate) struct BuildAccess {
    pub(crate) can_read: bool,
    /// Owners and admins see the error output and may queue a build again.
    pub(crate) is_owner: bool,
}

impl DocBuildResponse {
    /// The build as seen by a user with `access`, who may queue builds in
    /// general if `can_queue_builds`.
    pub(crate) fn new(
        recorded: RecordedDocBuild,
        access: BuildAccess,
        can_queue_builds: bool,
    ) -> Self {
        let build = recorded.build;
        Self {
            id: recorded.id,
            name: build.name.to_string(),
            version: build.version.to_string(),
            status: build.status,
            started: build.started,
            finished: build.finished,
            error: build.error.filter(|_| access.is_owner),
            can_rebuild: access.is_owner && can_queue_builds,
        }
    }
}
