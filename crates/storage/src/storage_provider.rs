use axum::async_trait;
use std::path::Path;
use tokio::fs::File;

#[async_trait]
pub trait StorageProvider: Send + Sync {
    type Err;
    type StoredFile;
    async fn open_file(&self, file_path: &Path) -> Result<Self::StoredFile, Self::Err>;
    async fn open_or_create_file(&self, file_path: &Path) -> Result<Self::StoredFile, Self::Err>;
    async fn read_file(&self, file: &mut Self::StoredFile) -> Result<String, Self::Err>;
    async fn read(&self, path: &Path) -> Result<Vec<u8>, Self::Err>;
}

pub mod mock {
    use super::*;
    use mockall::predicate::*;
    use mockall::*;

    use crate::storage_error::StorageError;

    mock! {
        pub Storage {}

        #[async_trait]
        impl StorageProvider for Storage {
                type Err = StorageError;
                type StoredFile = File;
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
