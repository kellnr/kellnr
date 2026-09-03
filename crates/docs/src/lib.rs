pub mod api;
mod doc_archive;
pub mod doc_queue;
pub mod doc_queue_response;
pub mod docs_error;
pub mod upload;
pub mod upload_response;

use std::convert::TryFrom;

use kellnr_common::version::Version;
use kellnr_storage::docs_storage::DocsStorage;

use crate::docs_error::DocsError;

pub async fn get_latest_doc_url(
    crate_name: &str,
    docs_storage: &DocsStorage,
    path_prefix: &str,
) -> Option<String> {
    let version = get_latest_version_with_doc(crate_name, docs_storage).await?;
    Some(compute_doc_url(crate_name, &version, path_prefix))
}

pub async fn get_doc_url(
    crate_name: &str,
    crate_version: &Version,
    docs_storage: &DocsStorage,
    path_prefix: &str,
) -> Option<String> {
    if doc_exists(crate_name, &crate_version.to_string(), docs_storage).await {
        Some(compute_doc_url(crate_name, crate_version, path_prefix))
    } else {
        None
    }
}

pub fn compute_doc_url(crate_name: &str, crate_version: &Version, path_prefix: &str) -> String {
    let docs_name = crate_name_to_docs_name(crate_name);
    let path_prefix = path_prefix.trim();
    format!("{path_prefix}/docs/{crate_name}/{crate_version}/doc/{docs_name}/index.html")
}

fn crate_name_to_docs_name(crate_name: &str) -> String {
    // Cargo replaces the `-` with `_` in the crate name when
    // docs are generated. As such, the docs folder name is not "foo-bar" but "foo_bar".
    crate_name.replace('-', "_")
}

async fn doc_exists(crate_name: &str, crate_version: &str, docs_storage: &DocsStorage) -> bool {
    let docs_name = crate_name_to_docs_name(crate_name);
    let key = DocsStorage::file_key(
        crate_name,
        crate_version,
        &format!("doc/{docs_name}/index.html"),
    );
    docs_storage.exists(&key).await.unwrap_or(false)
}

async fn get_latest_version_with_doc(
    crate_name: &str,
    docs_storage: &DocsStorage,
) -> Option<Version> {
    let mut versions: Vec<Version> = docs_storage
        .version_candidates(crate_name)
        .await
        .ok()?
        .into_iter()
        .flat_map(|v| Version::try_from(&v))
        .collect();

    // Sort and reverse the order such that the biggest version
    // for which docs exist will be returned.
    versions.sort();
    versions.reverse();

    for version in versions {
        if doc_exists(crate_name, &version.to_string(), docs_storage).await {
            return Some(version);
        }
    }
    None
}

pub async fn delete(
    crate_name: &str,
    crate_version: &str,
    docs_storage: &DocsStorage,
) -> Result<(), DocsError> {
    docs_storage
        .delete_prefix(&DocsStorage::version_prefix(crate_name, crate_version))
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use kellnr_storage::fs_storage::FSStorage;

    use super::*;

    #[test]
    fn compute_doc_url_without_path_prefix() {
        let version = Version::try_from("1.0.0").unwrap();
        let url = compute_doc_url("my-crate", &version, "");
        assert_eq!(url, "/docs/my-crate/1.0.0/doc/my_crate/index.html");
    }

    #[test]
    fn compute_doc_url_with_path_prefix() {
        let version = Version::try_from("1.0.0").unwrap();
        let url = compute_doc_url("my-crate", &version, "/kellnr");
        assert_eq!(url, "/kellnr/docs/my-crate/1.0.0/doc/my_crate/index.html");
    }

    #[test]
    fn compute_doc_url_trims_whitespace_from_path_prefix() {
        let version = Version::try_from("1.0.0").unwrap();
        let url = compute_doc_url("my-crate", &version, "  /kellnr  ");
        assert_eq!(url, "/kellnr/docs/my-crate/1.0.0/doc/my_crate/index.html");
    }

    #[test]
    fn compute_doc_url_replaces_hyphen_with_underscore_in_docs_name() {
        let version = Version::try_from("2.0.0-beta1").unwrap();
        let url = compute_doc_url("foo-bar-baz", &version, "");
        assert_eq!(
            url,
            "/docs/foo-bar-baz/2.0.0-beta1/doc/foo_bar_baz/index.html"
        );
    }

    fn docs_storage() -> (tempfile::TempDir, DocsStorage) {
        let dir = tempfile::tempdir().unwrap();
        let storage = FSStorage::new(dir.path().to_str().unwrap()).unwrap();
        (dir, DocsStorage::new(Box::new(storage)))
    }

    #[tokio::test]
    async fn doc_exists_true_when_index_html_present() {
        let (_dir, docs) = docs_storage();
        let key = DocsStorage::file_key("my-crate", "1.0.0", "doc/my_crate/index.html");
        docs.put(&key, Bytes::from_static(b"x")).await.unwrap();

        assert!(doc_exists("my-crate", "1.0.0", &docs).await);
    }

    #[tokio::test]
    async fn doc_exists_false_when_missing() {
        let (_dir, docs) = docs_storage();
        assert!(!doc_exists("my-crate", "1.0.0", &docs).await);
    }

    #[tokio::test]
    async fn get_latest_version_with_doc_returns_highest_version() {
        let (_dir, docs) = docs_storage();
        for v in ["1.0.0", "2.0.0", "1.5.0"] {
            let key = DocsStorage::file_key("my-crate", v, "doc/my_crate/index.html");
            docs.put(&key, Bytes::from_static(b"x")).await.unwrap();
        }

        let latest = get_latest_version_with_doc("my-crate", &docs)
            .await
            .unwrap();
        assert_eq!(latest.to_string(), "2.0.0");
    }

    #[tokio::test]
    async fn get_latest_version_with_doc_none_when_no_versions() {
        let (_dir, docs) = docs_storage();
        assert!(
            get_latest_version_with_doc("my-crate", &docs)
                .await
                .is_none()
        );
    }

    #[tokio::test]
    async fn delete_removes_only_targeted_version() {
        let (_dir, docs) = docs_storage();
        let v1 = DocsStorage::file_key("my-crate", "1.0.0", "doc/my_crate/index.html");
        let v2 = DocsStorage::file_key("my-crate", "2.0.0", "doc/my_crate/index.html");
        docs.put(&v1, Bytes::from_static(b"a")).await.unwrap();
        docs.put(&v2, Bytes::from_static(b"b")).await.unwrap();

        delete("my-crate", "1.0.0", &docs).await.unwrap();

        assert!(!docs.exists(&v1).await.unwrap());
        assert!(docs.exists(&v2).await.unwrap());
    }
}
