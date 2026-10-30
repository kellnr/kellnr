use kellnr_db::DocQueueEntry;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Response containing documentation build queue
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, ToSchema)]
pub struct DocQueueResponse {
    /// List of crates in the documentation build queue
    pub(crate) queue: Vec<DocQueueEntryResponse>,
}

/// Entry in the documentation build queue
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, ToSchema)]
pub struct DocQueueEntryResponse {
    /// Crate name
    pub(crate) name: String,
    /// Crate version
    pub(crate) version: String,
}

impl From<Vec<DocQueueEntry>> for DocQueueResponse {
    fn from(entries: Vec<DocQueueEntry>) -> Self {
        Self {
            queue: entries
                .into_iter()
                .map(|e| DocQueueEntryResponse {
                    name: e.name,
                    version: e.version,
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn doc_queue_response_from_doc_queue_entry() {
        let doc_queue = vec![
            DocQueueEntry {
                id: 0,
                name: "crate1".to_string(),
                version: "0.0.1".to_string(),
                path: PathBuf::default(),
            },
            DocQueueEntry {
                id: 1,
                name: "crate2".to_string(),
                version: "0.0.2".to_string(),
                path: PathBuf::default(),
            },
        ];

        let doc_queue_response = DocQueueResponse::from(doc_queue);

        assert_eq!(
            DocQueueResponse {
                queue: vec![
                    DocQueueEntryResponse {
                        name: "crate1".to_string(),
                        version: "0.0.1".to_string()
                    },
                    DocQueueEntryResponse {
                        name: "crate2".to_string(),
                        version: "0.0.2".to_string()
                    }
                ]
            },
            doc_queue_response
        );
    }
}
