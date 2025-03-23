use crate::storage_error::StorageError;
use async_trait::async_trait;
use bytes::Bytes;

#[async_trait]
pub trait Storage {
    async fn get(&self, key: &str) -> Result<Bytes, StorageError>;
    async fn put(&self, key: &str, object: Bytes) -> Result<(), StorageError>;
    async fn delete(&self, key: &str) -> Result<(), StorageError>;
}
