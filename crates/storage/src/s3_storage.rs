use crate::storage_provider::StorageProvider;
use aws_sdk_s3::config::{Builder, Credentials};
use aws_sdk_s3::error::SdkError;
use aws_sdk_s3::operation::get_object::GetObjectError;
use aws_sdk_s3::operation::put_object::PutObjectError;
use aws_sdk_s3::primitives::{ByteStream, ByteStreamError};
use aws_sdk_s3::{config::Region, Client};
use aws_smithy_runtime_api::http::Response;
use axum::async_trait;
use bytes::Bytes;
use std::env::var;
use std::path::{Path, PathBuf};
use std::string::FromUtf8Error;
use std::sync::Arc;
use thiserror::Error;

#[derive(Clone)]
pub struct S3Storage {
    pub(crate) client: Client,
    pub(crate) bucket: String,
}

pub struct S3File {
    pub path: PathBuf,
    pub content: Bytes,
    s3: Arc<S3Storage>,
}

/// This is quite horrific but the StorageProvider trait forces this on us as it gets a file 
/// back and can do whatever it wants with said file without explictly committing changes.
impl Drop for S3File {
    fn drop(&mut self) {
        futures::executor::block_on(async {
            if let Err(e) = self
                .s3
                .client
                .put_object()
                .bucket(self.s3.bucket.clone())
                .key(self.path.to_string_lossy())
                .body(ByteStream::from(self.content.clone()))
                .send()
                .await
            {
                eprintln!(
                    "Failed to write file {} on drop. Error {:?}",
                    self.path.to_string_lossy(),
                    e
                );
            }
        })
    }
}

#[derive(Debug, Error)]
pub enum S3Error {
    #[error(transparent)]
    FileReadError(#[from] SdkError<GetObjectError, Response>),
    #[error(transparent)]
    FileWriteError(#[from] SdkError<PutObjectError, Response>),
    #[error(transparent)]
    Utf8Error(#[from] FromUtf8Error),
    #[error(transparent)]
    ByteStreamError(#[from] ByteStreamError),
}

impl S3Storage {
    pub fn new() -> Self {
        let endpoint = var("S3_ENDPOINT").expect("Expected to find S3_ENDPOINT envvar");
        let region = Region::new(var("S3_REGION").expect("Expected to find S3_REGION envvar"));
        let bucket = var("S3_BUCKET").expect("Expected to find S3_BUCKET envvar");
        let access_key = var("S3_ACCESS_KEY").expect("Expected to find S3_ACCESS_KEY envvar");
        let secret_key = var("S3_SECRET_KEY").expect("Expected to find S3_SECRET_KEY envvar");

        let config = Builder::new()
            .region(region)
            .endpoint_url(endpoint)
            .credentials_provider(Credentials::new(
                access_key, secret_key, None, None, "static",
            ))
            .build();

        let client = Client::from_conf(config);
        Self { client, bucket }
    }
}

#[async_trait]
impl StorageProvider for S3Storage {
    type Err = S3Error;
    type StoredFile = S3File;

    async fn open_file(&self, path: &Path) -> Result<S3File, S3Error> {
        let content = self
            .client
            .get_object()
            .bucket(self.bucket.clone())
            .key(path.to_string_lossy())
            .send()
            .await?
            .body
            .collect()
            .await?
            .into_bytes();

        Ok(S3File {
            path: path.to_path_buf(),
            content,
            s3: Arc::new(self.clone()),
        })
    }

    async fn open_or_create_file(&self, file_path: &Path) -> Result<S3File, S3Error> {
        match self.open_file(&file_path).await {
            Ok(r) => Ok(r),
            Err(S3Error::FileReadError(sdk_error)) => {
                if let Some(GetObjectError::NoSuchKey(_)) = sdk_error.as_service_error() {
                    const EMPTY: &[u8] = &[];
                    let _put_output = self
                        .client
                        .put_object()
                        .bucket(self.bucket.clone())
                        .key(file_path.to_string_lossy())
                        .body(ByteStream::from_static(EMPTY))
                        .send()
                        .await?;
                    Ok(S3File {
                        path: file_path.to_path_buf(),
                        content: Bytes::new(),
                        s3: Arc::new(self.clone()),
                    })
                } else {
                    Err(S3Error::FileReadError(sdk_error))
                }
            }
            Err(e) => Err(e),
        }
    }

    async fn read_file(&self, file: &mut S3File) -> Result<String, S3Error> {
        String::from_utf8(file.content.to_vec()).map_err(S3Error::Utf8Error)
    }

    async fn read(&self, path: &Path) -> Result<Vec<u8>, S3Error> {
        Ok(self.open_file(path).await?.content.to_vec())
    }
}
