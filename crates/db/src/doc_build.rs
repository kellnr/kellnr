use chrono::{DateTime, Utc};
use kellnr_common::original_name::OriginalName;
use kellnr_common::version::Version;
use serde::{Deserialize, Serialize};
use tracing::warn;

/// The result of a finished rustdoc build of a crate version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocBuild {
    pub name: OriginalName,
    pub version: Version,
    pub status: DocBuildStatus,
    /// Why the build failed, `None` for successful builds.
    pub error: Option<String>,
    pub started: DateTime<Utc>,
    pub finished: DateTime<Utc>,
}

/// A docs build read from the build history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedDocBuild {
    pub id: i64,
    pub build: DocBuild,
}

/// Outcome of a rustdoc build.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DocBuildStatus {
    /// The docs were built and published.
    Succeeded,
    /// The build or the upload of the docs failed.
    Failed,
    /// The build took longer than `docs.build_timeout_seconds` and was killed.
    TimedOut,
}

impl DocBuildStatus {
    /// The name the status is stored with in the database.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::TimedOut => "timed_out",
        }
    }

    /// Reads a status stored in the database. Only the names of the variants
    /// are ever stored; anything else is shown as failed.
    pub(crate) fn from_db(status: &str) -> Self {
        match status {
            "succeeded" => Self::Succeeded,
            "failed" => Self::Failed,
            "timed_out" => Self::TimedOut,
            other => {
                warn!("Unknown docs build status {other:?}, treating it as failed");
                Self::Failed
            }
        }
    }
}
