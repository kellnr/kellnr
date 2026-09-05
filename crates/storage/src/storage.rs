use async_trait::async_trait;
use bytes::Bytes;
use chrono::{DateTime, Utc};

use crate::storage_error::StorageError;

/// An object's bytes alongside the metadata needed for HTTP conditional-GET
/// (`ETag`/`Last-Modified`) support.
pub struct ObjectWithMeta {
    pub bytes: Bytes,
    pub e_tag: Option<String>,
    pub last_modified: DateTime<Utc>,
}

#[async_trait]
pub trait Storage {
    async fn get(&self, key: &str) -> Result<Bytes, StorageError>;
    /// Like [`Storage::get`], but also returns the `ETag`/`Last-Modified`
    /// metadata `object_store` already computes for every backend, so
    /// callers can support conditional-GET without a second round-trip.
    async fn get_with_meta(&self, key: &str) -> Result<ObjectWithMeta, StorageError>;
    async fn put(&self, key: &str, object: Bytes) -> Result<(), StorageError>;
    /// Like [`Storage::put`], but overwrites an existing key instead of
    /// failing. Needed for content that is republished in place (e.g. docs),
    /// unlike crate tarballs/toolchain archives which are immutable.
    async fn put_overwrite(&self, key: &str, object: Bytes) -> Result<(), StorageError>;
    async fn delete(&self, key: &str) -> Result<(), StorageError>;
    async fn exists(&self, key: &str) -> Result<bool, StorageError>;
    /// Recursively list every key stored under `prefix`.
    async fn list(&self, prefix: &str) -> Result<Vec<String>, StorageError>;
}
