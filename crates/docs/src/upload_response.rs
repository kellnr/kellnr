use crate::compute_doc_url;
use common::original_name::OriginalName;
use common::version::Version;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocUploadResponse {
    pub message: String,
    pub url: String,
    pub crate_name: String,
    pub crate_version: String,
}

impl DocUploadResponse {
    pub fn new(message: String, crate_name: &OriginalName, crate_version: &Version) -> Self {
        Self {
            message,
            crate_name: crate_name.to_string(),
            crate_version: crate_version.to_string(),
            url: compute_doc_url(&crate_name.to_string(), crate_version),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryFrom;

    #[test]
    fn create_new_doc_upload_response_works() {
        let name = OriginalName::try_from("mycrate").unwrap();
        let version = Version::try_from("1.0.0-beta2").unwrap();
        let msg = String::from("Hello, this is the message.");

        let dur = DocUploadResponse::new(msg, &name, &version);

        assert_eq!(
            DocUploadResponse {
                message: String::from("Hello, this is the message."),
                url: String::from("/docs/mycrate/1.0.0-beta2/doc/mycrate/index.html"),
                crate_name: String::from("mycrate"),
                crate_version: String::from("1.0.0-beta2")
            },
            dur
        );
    }

    #[test]
    fn create_new_doc_upload_replace_hyphen_with_underscore() {
        let name = OriginalName::try_from("my-crate").unwrap();
        let version = Version::try_from("1.0.0").unwrap();
        let msg = String::from("Hello, this is the message.");

        let dur = DocUploadResponse::new(msg, &name, &version);

        assert_eq!(
            DocUploadResponse {
                message: String::from("Hello, this is the message."),
                url: String::from("/docs/my-crate/1.0.0/doc/my_crate/index.html"),
                crate_name: String::from("my-crate"),
                crate_version: String::from("1.0.0")
            },
            dur
        );
    }
}
