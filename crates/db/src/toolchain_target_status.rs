use serde::{Deserialize, Serialize};
use tracing::warn;

/// Processing status of an uploaded toolchain target, stored as its
/// lowercase name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ToolchainTargetStatus {
    /// The archive is being split into its components.
    Processing,
    /// The target can be installed.
    Ready,
    /// Splitting the archive failed or was interrupted.
    Failed,
}

impl ToolchainTargetStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Processing => "processing",
            Self::Ready => "ready",
            Self::Failed => "failed",
        }
    }

    /// Reads a status stored in the database. Only the names of the variants
    /// are ever stored; anything else is treated as failed, so the target is
    /// never offered for installation.
    pub(crate) fn from_db(status: &str) -> Self {
        match status {
            "processing" => Self::Processing,
            "ready" => Self::Ready,
            "failed" => Self::Failed,
            other => {
                warn!("Unknown toolchain target status {other:?}, treating it as failed");
                Self::Failed
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [ToolchainTargetStatus; 3] = [
        ToolchainTargetStatus::Processing,
        ToolchainTargetStatus::Ready,
        ToolchainTargetStatus::Failed,
    ];

    #[test]
    fn db_string_round_trip() {
        for status in ALL {
            assert_eq!(ToolchainTargetStatus::from_db(status.as_str()), status);
        }
        assert_eq!(ToolchainTargetStatus::Processing.as_str(), "processing");
        assert_eq!(ToolchainTargetStatus::Ready.as_str(), "ready");
        assert_eq!(ToolchainTargetStatus::Failed.as_str(), "failed");
    }

    #[test]
    fn json_is_the_db_string() {
        for status in ALL {
            assert_eq!(
                serde_json::to_value(status).unwrap(),
                serde_json::Value::String(status.as_str().to_string())
            );
        }
    }

    #[test]
    fn unknown_db_string_is_failed() {
        assert_eq!(
            ToolchainTargetStatus::from_db("Ready"),
            ToolchainTargetStatus::Failed
        );
    }
}
