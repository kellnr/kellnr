use bytes::Bytes;
use kellnr_storage::docs_storage::DocsStorage;
use kellnr_storage::fs_storage::FSStorage;
use kellnr_storage::storage::Storage;

fn temp_storage() -> (tempfile::TempDir, FSStorage) {
    let dir = tempfile::tempdir().unwrap();
    let storage = FSStorage::new(dir.path().to_str().unwrap()).unwrap();
    (dir, storage)
}

#[tokio::test]
async fn put_then_get_roundtrips() {
    let (_dir, storage) = temp_storage();
    storage
        .put("a/b.txt", Bytes::from_static(b"hello"))
        .await
        .unwrap();

    let data = storage.get("a/b.txt").await.unwrap();
    assert_eq!(data, Bytes::from_static(b"hello"));
}

#[tokio::test]
async fn get_with_meta_returns_bytes_etag_and_last_modified() {
    let (_dir, storage) = temp_storage();
    storage
        .put("a/b.txt", Bytes::from_static(b"hello"))
        .await
        .unwrap();

    let object = storage.get_with_meta("a/b.txt").await.unwrap();

    assert_eq!(object.bytes, Bytes::from_static(b"hello"));
    assert!(object.e_tag.is_some());
    assert!(object.last_modified <= chrono::Utc::now());
}

#[tokio::test]
async fn put_fails_if_key_already_exists() {
    let (_dir, storage) = temp_storage();
    storage
        .put("a/b.txt", Bytes::from_static(b"first"))
        .await
        .unwrap();

    let result = storage.put("a/b.txt", Bytes::from_static(b"second")).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn put_overwrite_replaces_existing_key() {
    let (_dir, storage) = temp_storage();
    storage
        .put("a/b.txt", Bytes::from_static(b"first"))
        .await
        .unwrap();

    storage
        .put_overwrite("a/b.txt", Bytes::from_static(b"second"))
        .await
        .unwrap();

    let data = storage.get("a/b.txt").await.unwrap();
    assert_eq!(data, Bytes::from_static(b"second"));
}

#[tokio::test]
async fn delete_removes_key() {
    let (_dir, storage) = temp_storage();
    storage
        .put("a/b.txt", Bytes::from_static(b"hello"))
        .await
        .unwrap();

    storage.delete("a/b.txt").await.unwrap();

    assert!(!storage.exists("a/b.txt").await.unwrap());
}

#[tokio::test]
async fn list_is_recursive_under_prefix() {
    let (_dir, storage) = temp_storage();
    storage
        .put("crate/1.0.0/doc/index.html", Bytes::from_static(b"a"))
        .await
        .unwrap();
    storage
        .put("crate/1.0.0/doc/nested/page.html", Bytes::from_static(b"b"))
        .await
        .unwrap();
    storage
        .put("crate/2.0.0/doc/index.html", Bytes::from_static(b"c"))
        .await
        .unwrap();
    storage
        .put("other-crate/1.0.0/doc/index.html", Bytes::from_static(b"d"))
        .await
        .unwrap();

    let mut keys = storage.list("crate/1.0.0/").await.unwrap();
    keys.sort();

    assert_eq!(
        keys,
        vec![
            "crate/1.0.0/doc/index.html".to_string(),
            "crate/1.0.0/doc/nested/page.html".to_string(),
        ]
    );
}

#[tokio::test]
async fn list_does_not_match_sibling_prefix_with_shared_string_prefix() {
    let (_dir, storage) = temp_storage();
    storage
        .put("foo/1.0.0/index.html", Bytes::from_static(b"a"))
        .await
        .unwrap();
    storage
        .put("foo-bar/1.0.0/index.html", Bytes::from_static(b"b"))
        .await
        .unwrap();

    let keys = storage.list("foo/").await.unwrap();

    assert_eq!(keys, vec!["foo/1.0.0/index.html".to_string()]);
}

#[tokio::test]
async fn list_returns_empty_for_missing_prefix() {
    let (_dir, storage) = temp_storage();
    let keys = storage.list("does-not-exist/").await.unwrap();
    assert!(keys.is_empty());
}

fn docs_storage() -> (tempfile::TempDir, DocsStorage) {
    let dir = tempfile::tempdir().unwrap();
    let storage = FSStorage::new(dir.path().to_str().unwrap()).unwrap();
    (dir, DocsStorage::new(Box::new(storage)))
}

#[tokio::test]
async fn docs_storage_put_overwrites_and_gets() {
    let (_dir, docs) = docs_storage();
    let key = DocsStorage::file_key("my-crate", "1.0.0", "doc/my_crate/index.html");

    docs.put(&key, Bytes::from_static(b"v1")).await.unwrap();
    docs.put(&key, Bytes::from_static(b"v2")).await.unwrap();

    let data = docs.get(&key).await.unwrap();
    assert_eq!(data, Bytes::from_static(b"v2"));
}

#[tokio::test]
async fn docs_storage_delete_prefix_removes_only_matching_version() {
    let (_dir, docs) = docs_storage();
    let v1 = DocsStorage::file_key("my-crate", "1.0.0", "doc/my_crate/index.html");
    let v2 = DocsStorage::file_key("my-crate", "2.0.0", "doc/my_crate/index.html");

    docs.put(&v1, Bytes::from_static(b"a")).await.unwrap();
    docs.put(&v2, Bytes::from_static(b"b")).await.unwrap();

    docs.delete_prefix(&DocsStorage::version_prefix("my-crate", "1.0.0"))
        .await
        .unwrap();

    assert!(!docs.exists(&v1).await.unwrap());
    assert!(docs.exists(&v2).await.unwrap());
}

#[tokio::test]
async fn docs_storage_version_candidates_lists_distinct_versions() {
    let (_dir, docs) = docs_storage();
    docs.put(
        &DocsStorage::file_key("my-crate", "1.0.0", "doc/my_crate/index.html"),
        Bytes::from_static(b"a"),
    )
    .await
    .unwrap();
    docs.put(
        &DocsStorage::file_key("my-crate", "1.0.0", "doc/my_crate/other.html"),
        Bytes::from_static(b"b"),
    )
    .await
    .unwrap();
    docs.put(
        &DocsStorage::file_key("my-crate", "2.0.0", "doc/my_crate/index.html"),
        Bytes::from_static(b"c"),
    )
    .await
    .unwrap();

    let mut versions = docs.version_candidates("my-crate").await.unwrap();
    versions.sort();

    assert_eq!(versions, vec!["1.0.0".to_string(), "2.0.0".to_string()]);
}
