pub mod api;
pub mod build_process;
mod doc_archive;
pub mod doc_queue;
pub mod doc_queue_response;
pub mod docs_error;
mod unpack;
pub mod upload;
pub mod upload_response;

use kellnr_common::original_name::OriginalName;
use kellnr_common::version::Version;
use kellnr_storage::docs_storage::{DocsName, DocsStorage};

use crate::docs_error::DocsError;

/// URL of the docs of a crate version. The docs are stored under the
/// lowercase crate name, but rustdoc keeps the casing of the crate name for
/// its output folder.
pub fn compute_doc_url(
    crate_name: &OriginalName,
    crate_version: &Version,
    path_prefix: &str,
) -> String {
    let folder = DocsName::from(crate_name);
    let docs_name = crate_name_to_docs_name(crate_name);
    format!("{path_prefix}/docs/{folder}/{crate_version}/doc/{docs_name}/index.html")
}

fn crate_name_to_docs_name(crate_name: &OriginalName) -> String {
    // Cargo replaces the `-` with `_` in the crate name when
    // docs are generated. As such, the docs folder name is not "foo-bar" but "foo_bar".
    crate_name.replace('-', "_")
}

async fn doc_exists(
    crate_name: &OriginalName,
    crate_version: &Version,
    docs_storage: &DocsStorage,
) -> bool {
    let docs_name = crate_name_to_docs_name(crate_name);
    let key = DocsStorage::file_key(
        &DocsName::from(crate_name),
        crate_version,
        &format!("doc/{docs_name}/index.html"),
    );
    docs_storage.exists(&key).await.unwrap_or(false)
}

pub async fn delete(
    crate_name: &OriginalName,
    crate_version: &Version,
    docs_storage: &DocsStorage,
) -> Result<(), DocsError> {
    docs_storage
        .delete_prefix(&DocsStorage::version_prefix(
            &DocsName::from(crate_name),
            crate_version,
        ))
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use kellnr_storage::fs_storage::FSStorage;
    use kellnr_storage::object_storage::MemoryStorage;

    use super::*;

    #[test]
    fn compute_doc_url_without_path_prefix() {
        let version = Version::try_from("1.0.0").unwrap();
        let url = compute_doc_url(&original("my-crate"), &version, "");
        assert_eq!(url, "/docs/my-crate/1.0.0/doc/my_crate/index.html");
    }

    #[test]
    fn compute_doc_url_with_path_prefix() {
        let version = Version::try_from("1.0.0").unwrap();
        let url = compute_doc_url(&original("my-crate"), &version, "/kellnr");
        assert_eq!(url, "/kellnr/docs/my-crate/1.0.0/doc/my_crate/index.html");
    }

    #[test]
    fn compute_doc_url_replaces_hyphen_with_underscore_in_docs_name() {
        let version = Version::try_from("2.0.0-beta1").unwrap();
        let url = compute_doc_url(&original("foo-bar-baz"), &version, "");
        assert_eq!(
            url,
            "/docs/foo-bar-baz/2.0.0-beta1/doc/foo_bar_baz/index.html"
        );
    }

    #[test]
    fn compute_doc_url_keeps_casing_of_docs_name_only() {
        let version = Version::try_from("1.0.0").unwrap();
        let url = compute_doc_url(&original("My-Crate"), &version, "");
        assert_eq!(url, "/docs/my-crate/1.0.0/doc/My_Crate/index.html");
    }

    fn original(name: &str) -> OriginalName {
        OriginalName::try_from(name).unwrap()
    }

    fn docs_storage() -> (tempfile::TempDir, DocsStorage) {
        let dir = tempfile::tempdir().unwrap();
        let storage = FSStorage::new(dir.path().to_str().unwrap()).unwrap();
        (dir, DocsStorage::new(Box::new(storage)))
    }

    #[tokio::test]
    async fn doc_exists_true_when_index_html_present() {
        let (_dir, docs) = docs_storage();
        let key = DocsStorage::file_key(
            &DocsName::from(&original("my-crate")),
            &Version::from_unchecked_str("1.0.0"),
            "doc/my_crate/index.html",
        );
        docs.put(&key, Bytes::from_static(b"x")).await.unwrap();

        assert!(
            doc_exists(
                &original("my-crate"),
                &Version::from_unchecked_str("1.0.0"),
                &docs
            )
            .await
        );
    }

    #[tokio::test]
    async fn doc_exists_false_when_missing() {
        let (_dir, docs) = docs_storage();
        assert!(
            !doc_exists(
                &original("my-crate"),
                &Version::from_unchecked_str("1.0.0"),
                &docs
            )
            .await
        );
    }

    #[tokio::test]
    async fn doc_exists_for_crate_with_uppercase_letters() {
        let docs = DocsStorage::new(Box::new(MemoryStorage::new()));
        let key = DocsStorage::file_key(
            &DocsName::from(&original("my-crate")),
            &Version::from_unchecked_str("1.0.0"),
            "doc/My_Crate/index.html",
        );
        docs.put(&key, Bytes::from_static(b"x")).await.unwrap();

        assert!(
            doc_exists(
                &original("My-Crate"),
                &Version::from_unchecked_str("1.0.0"),
                &docs
            )
            .await
        );
    }

    #[tokio::test]
    async fn delete_removes_only_targeted_version() {
        let (_dir, docs) = docs_storage();
        let v1 = DocsStorage::file_key(
            &DocsName::from(&original("my-crate")),
            &Version::from_unchecked_str("1.0.0"),
            "doc/my_crate/index.html",
        );
        let v2 = DocsStorage::file_key(
            &DocsName::from(&original("my-crate")),
            &Version::from_unchecked_str("2.0.0"),
            "doc/my_crate/index.html",
        );
        docs.put(&v1, Bytes::from_static(b"a")).await.unwrap();
        docs.put(&v2, Bytes::from_static(b"b")).await.unwrap();

        delete(
            &original("my-crate"),
            &Version::from_unchecked_str("1.0.0"),
            &docs,
        )
        .await
        .unwrap();

        assert!(!docs.exists(&v1).await.unwrap());
        assert!(docs.exists(&v2).await.unwrap());
    }
}
