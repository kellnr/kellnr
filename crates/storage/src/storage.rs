use crate::{storage_error::StorageError, storage_provider::StorageProvider};
use axum::async_trait;
use std::path::Path;
use tokio::{
    fs::{read, File, OpenOptions},
    io::AsyncReadExt,
};

#[derive(Clone)]
pub struct Storage {}

impl Storage {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for Storage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StorageProvider for Storage {
    async fn open_file(&self, file_path: &Path) -> Result<File, StorageError> {
        if file_path.exists() {
            let file = OpenOptions::new()
                .read(true)
                .append(true)
                .open(&file_path)
                .await
                .map_err(|e| StorageError::OpenFileOnStorage(file_path.to_path_buf(), e))?;
            Ok(file)
        } else {
            Err(StorageError::FileDoesNotExist(file_path.to_path_buf()))
        }
    }

    async fn open_or_create_file(&self, file_path: &Path) -> Result<File, StorageError> {
        if file_path.exists() {
            self.open_file(file_path).await
        } else {
            let file = OpenOptions::new()
                .read(true)
                .append(true)
                .create(true)
                .open(&file_path)
                .await
                .map_err(|e| StorageError::CreateFile(e, file_path.to_path_buf()))?;
            Ok(file)
        }
    }

    async fn read_file(&self, file: &mut File) -> Result<String, StorageError> {
        let mut content = String::new();
        file.read_to_string(&mut content)
            .await
            .map_err(|e| StorageError::ReadFileHandle(e))?;

        Ok(content)
    }

    async fn read(&self, path: &Path) -> Result<Vec<u8>, StorageError> {
        read(&path)
            .await
            .map_err(|e| StorageError::ReadFile(path.to_path_buf(), e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn read_returns_byte_vec() {
        let storage = Storage::new();

        let bytes = storage
            .read(&PathBuf::from(
                &PathBuf::from("../test_data").join("pub_data.bin"),
            ))
            .await;

        assert!(bytes.is_ok());
        assert_eq!(1786, bytes.unwrap().len())
    }

    #[tokio::test]
    async fn read_not_found() {
        let storage = Storage::new();

        let bytes = storage
            .read(&PathBuf::from(
                &PathBuf::from("../test_data").join("doesnotexist.txt"),
            ))
            .await;

        assert!(bytes.is_err());
    }
}
