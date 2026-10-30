use std::collections::HashSet;
use std::path::{Path, PathBuf};

use bytes::Bytes;
use kellnr_storage::docs_storage::DocsStorage;

use crate::docs_error::DocsError;

/// Upload every regular file under `local_root` to `docs_storage` under the
/// `{crate_name}/{version}/{key_prefix}/` prefix, then delete any pre-existing
/// key under `{crate_name}/{version}/` that wasn't just uploaded.
///
/// Symlinks are never followed, so only files that were actually written into
/// `local_root` can end up in the docs storage.
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
    let mut uploaded_keys = HashSet::new();

    for file_path in regular_files(local_root)? {
        let relative =
            file_path
                .strip_prefix(local_root)
                .map_err(|_| DocsError::PathPrefixMismatch {
                    path: file_path.display().to_string(),
                    root: local_root.display().to_string(),
                })?;
        let relative_key = relative.to_string_lossy().replace('\\', "/");
        let relative_key = if key_prefix.is_empty() {
            relative_key
        } else {
            format!("{key_prefix}/{relative_key}")
        };
        let key = DocsStorage::file_key(crate_name, version, &relative_key);

        let data = tokio::fs::read(&file_path).await?;
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

/// Collect all regular files below `root` without following symlinks.
fn regular_files(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    if !std::fs::symlink_metadata(root)?.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{} is not a directory", root.display()),
        ));
    }
    let mut files = Vec::new();
    let mut dirs = vec![root.to_path_buf()];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            // `DirEntry::file_type` does not follow symlinks.
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                dirs.push(entry.path());
            } else if file_type.is_file() {
                files.push(entry.path());
            }
        }
    }
    Ok(files)
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
    async fn upload_keeps_files_with_special_characters() {
        let (_storage_dir, docs) = docs_storage();
        let local = tempfile::tempdir().unwrap();
        for file in ["a~b.html", "x%20y.html"] {
            tokio::fs::write(local.path().join(file), file)
                .await
                .unwrap();
        }

        upload_dir_and_prune(local.path(), "", "my-crate", "1.0.0", &docs)
            .await
            .unwrap();

        for file in ["a~b.html", "x%20y.html"] {
            let key = DocsStorage::file_key("my-crate", "1.0.0", file);
            assert_eq!(docs.get(&key).await.unwrap(), Bytes::from(file), "{file}");
        }
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

    #[cfg(unix)]
    #[tokio::test]
    async fn upload_skips_symlinked_files_and_directories() {
        let (_storage_dir, docs) = docs_storage();
        let outside = tempfile::tempdir().unwrap();
        tokio::fs::write(outside.path().join("db.sqlite"), b"secret")
            .await
            .unwrap();

        let local = tempfile::tempdir().unwrap();
        tokio::fs::write(local.path().join("index.html"), b"root")
            .await
            .unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("db.sqlite"),
            local.path().join("db.html"),
        )
        .unwrap();
        std::os::unix::fs::symlink(outside.path(), local.path().join("linked_dir")).unwrap();

        upload_dir_and_prune(local.path(), "", "my-crate", "1.0.0", &docs)
            .await
            .unwrap();

        let keys = docs
            .list(&DocsStorage::version_prefix("my-crate", "1.0.0"))
            .await
            .unwrap();
        assert_eq!(
            keys,
            vec![DocsStorage::file_key("my-crate", "1.0.0", "index.html")]
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn upload_rejects_symlinked_root() {
        let (_storage_dir, docs) = docs_storage();
        let outside = tempfile::tempdir().unwrap();
        tokio::fs::write(outside.path().join("index.html"), b"secret")
            .await
            .unwrap();
        let local = tempfile::tempdir().unwrap();
        let root = local.path().join("doc");
        std::os::unix::fs::symlink(outside.path(), &root).unwrap();

        let result = upload_dir_and_prune(&root, "", "my-crate", "1.0.0", &docs).await;

        assert!(result.is_err());
    }
}
