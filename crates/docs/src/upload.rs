use std::collections::HashSet;
use std::path::Path;

use bytes::Bytes;
use kellnr_storage::docs_storage::DocsStorage;

use crate::docs_error::DocsError;

/// Upload every file under `local_root` to `docs_storage` under the
/// `{crate_name}/{version}/{key_prefix}/` prefix, then delete any pre-existing
/// key under `{crate_name}/{version}/` that wasn't just uploaded.
///
/// Upload-then-prune (not delete-then-upload) so a partial/failed upload
/// never leaves a doc version with zero pages.
///
/// `key_prefix` lets the auto-generation queue (which walks `target/doc`
/// directly) and the manual-upload path (whose zip already contains a
/// top-level `doc/` folder) both land under `{crate}/{version}/doc/...`,
/// matching the layout `compute_doc_url` expects. Pass `""` when `local_root`
/// already contains the full relative structure.
pub async fn upload_dir_and_prune(
    local_root: &Path,
    key_prefix: &str,
    crate_name: &str,
    version: &str,
    docs_storage: &DocsStorage,
) -> Result<(), DocsError> {
    let content = fs_extra::dir::get_dir_content(local_root)?;
    let mut uploaded_keys = HashSet::new();

    for file in &content.files {
        let file_path = Path::new(file);
        let relative =
            file_path
                .strip_prefix(local_root)
                .map_err(|_| DocsError::PathPrefixMismatch {
                    path: file.clone(),
                    root: local_root.display().to_string(),
                })?;
        let relative_key = relative.to_string_lossy().replace('\\', "/");
        let relative_key = if key_prefix.is_empty() {
            relative_key
        } else {
            format!("{key_prefix}/{relative_key}")
        };
        let key = DocsStorage::file_key(crate_name, version, &relative_key);

        let data = tokio::fs::read(file_path).await?;
        docs_storage.put(&key, Bytes::from(data)).await?;
        uploaded_keys.insert(key);
    }

    let prefix = DocsStorage::version_prefix(crate_name, version);
    for existing_key in docs_storage.list(&prefix).await? {
        if !uploaded_keys.contains(&existing_key) {
            docs_storage.delete(&existing_key).await?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use kellnr_storage::fs_storage::FSStorage;

    use super::*;

    fn docs_storage() -> (tempfile::TempDir, DocsStorage) {
        let dir = tempfile::tempdir().unwrap();
        let storage = FSStorage::new(dir.path().to_str().unwrap()).unwrap();
        (dir, DocsStorage::new(Box::new(storage)))
    }

    #[tokio::test]
    async fn upload_preserves_directory_structure() {
        let (_storage_dir, docs) = docs_storage();
        let local = tempfile::tempdir().unwrap();
        tokio::fs::write(local.path().join("index.html"), b"root")
            .await
            .unwrap();
        tokio::fs::create_dir(local.path().join("nested"))
            .await
            .unwrap();
        tokio::fs::write(local.path().join("nested/page.html"), b"nested")
            .await
            .unwrap();

        upload_dir_and_prune(local.path(), "", "my-crate", "1.0.0", &docs)
            .await
            .unwrap();

        assert_eq!(
            docs.get(&DocsStorage::file_key("my-crate", "1.0.0", "index.html"))
                .await
                .unwrap(),
            Bytes::from_static(b"root")
        );
        assert_eq!(
            docs.get(&DocsStorage::file_key(
                "my-crate",
                "1.0.0",
                "nested/page.html"
            ))
            .await
            .unwrap(),
            Bytes::from_static(b"nested")
        );
    }

    #[tokio::test]
    async fn upload_prunes_stale_keys_from_previous_build() {
        let (_storage_dir, docs) = docs_storage();
        let stale_key = DocsStorage::file_key("my-crate", "1.0.0", "removed.html");
        docs.put(&stale_key, Bytes::from_static(b"old"))
            .await
            .unwrap();

        let local = tempfile::tempdir().unwrap();
        tokio::fs::write(local.path().join("index.html"), b"new")
            .await
            .unwrap();

        upload_dir_and_prune(local.path(), "", "my-crate", "1.0.0", &docs)
            .await
            .unwrap();

        assert!(!docs.exists(&stale_key).await.unwrap());
        assert!(
            docs.exists(&DocsStorage::file_key("my-crate", "1.0.0", "index.html"))
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn upload_leaves_other_versions_untouched() {
        let (_storage_dir, docs) = docs_storage();
        let other_version_key = DocsStorage::file_key("my-crate", "2.0.0", "index.html");
        docs.put(&other_version_key, Bytes::from_static(b"kept"))
            .await
            .unwrap();

        let local = tempfile::tempdir().unwrap();
        tokio::fs::write(local.path().join("index.html"), b"new")
            .await
            .unwrap();

        upload_dir_and_prune(local.path(), "", "my-crate", "1.0.0", &docs)
            .await
            .unwrap();

        assert!(docs.exists(&other_version_key).await.unwrap());
    }

    #[tokio::test]
    async fn upload_nests_files_under_key_prefix() {
        let (_storage_dir, docs) = docs_storage();
        let local = tempfile::tempdir().unwrap();
        tokio::fs::write(local.path().join("index.html"), b"root")
            .await
            .unwrap();

        upload_dir_and_prune(local.path(), "doc", "my-crate", "1.0.0", &docs)
            .await
            .unwrap();

        assert_eq!(
            docs.get(&DocsStorage::file_key(
                "my-crate",
                "1.0.0",
                "doc/index.html"
            ))
            .await
            .unwrap(),
            Bytes::from_static(b"root")
        );
    }
}
