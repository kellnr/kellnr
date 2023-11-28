use crate::storage_provider::StorageProvider;
use anyhow::{bail, Context, Error, Result};
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
    async fn open_file(&self, file_path: &Path) -> Result<File, Error> {
        if file_path.exists() {
            let file = OpenOptions::new()
                .read(true)
                .append(true)
                .open(&file_path)
                .await
                .with_context(|| format!("Unable to open file on index: {:?}", &file_path))?;
            Ok(file)
        } else {
            bail!("File does not exist: {:?}", &file_path)
        }
    }

    async fn open_or_create_file(&self, file_path: &Path) -> Result<File> {
        if file_path.exists() {
            self.open_file(file_path).await
        } else {
            let file = OpenOptions::new()
                .read(true)
                .append(true)
                .create(true)
                .open(&file_path)
                .await
                .with_context(|| format!("Unable to create file on index: {:?}", &file_path))?;
            Ok(file)
        }
    }

    async fn read_file(&self, file: &mut File) -> Result<String> {
        let mut content = String::new();
        file.read_to_string(&mut content)
            .await
            .with_context(|| "Unable to read existing file")?;

        Ok(content)
    }

    async fn read(&self, path: &Path) -> Result<Vec<u8>> {
        read(&path)
            .await
            .with_context(|| format!("Unable to read raw file: {:?}", &path.to_string_lossy()))
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
