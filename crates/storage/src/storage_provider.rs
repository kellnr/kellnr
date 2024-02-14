use axum::async_trait;
use std::path::Path;
use tokio::fs::File;

use crate::storage_error::StorageError;

#[async_trait]
pub trait StorageProvider: Send + Sync {
    async fn open_file(&self, file_path: &Path) -> Result<File, StorageError>;
    async fn open_or_create_file(&self, file_path: &Path) -> Result<File, StorageError>;
    async fn read_file(&self, file: &mut File) -> Result<String, StorageError>;
    async fn read(&self, path: &Path) -> Result<Vec<u8>, StorageError>;
}

pub mod mock {
    use super::*;
    use mockall::predicate::*;
    use mockall::*;

    mock! {
        pub Storage {}

        #[async_trait]
        impl StorageProvider for Storage {
                async fn open_file(&self, _file_path: &Path) -> Result<File, StorageError> {
                    unimplemented!()
                }

                async fn open_or_create_file(&self, _file_path: &Path) -> Result<File, StorageError> {
                    unimplemented!()
                }

                async fn read_file(&self, _file: &mut File) -> Result<String, StorageError> {
                    unimplemented!()
                }

                async fn read(&self, _path: &Path) -> Result<Vec<u8>, StorageError> {
                    unimplemented!()
                }
            }
    }
}
