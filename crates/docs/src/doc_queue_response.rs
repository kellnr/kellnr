use db::DocQueueEntry;
use json_payload::json_payload;

#[json_payload]
pub struct DocQueueResponse {
    pub(crate) queue: Vec<DocQueueEntryResponse>,
}

#[json_payload]
pub struct DocQueueEntryResponse {
    pub(crate) name: String,
    pub(crate) version: String,
}

impl From<Vec<DocQueueEntry>> for DocQueueResponse {
    fn from(entries: Vec<DocQueueEntry>) -> Self {
        Self {
            queue: entries
                .into_iter()
                .map(|e| DocQueueEntryResponse {
                    name: e.krate.to_string(),
                    version: e.version,
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::normalized_name::NormalizedName;

    #[test]
    fn doc_queue_response_from_doc_queue_entry() {
        let doc_queue = vec![
            DocQueueEntry {
                id: 0,
                krate: NormalizedName::from_unchecked("crate1".to_string()),
                version: "0.0.1".to_string(),
                path: Default::default(),
            },
            DocQueueEntry {
                id: 1,
                krate: NormalizedName::from_unchecked("crate2".to_string()),
                version: "0.0.2".to_string(),
                path: Default::default(),
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
