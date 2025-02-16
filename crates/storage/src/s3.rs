use std::path::PathBuf;

use bytes::Bytes;
use object_store::{
    aws::{AmazonS3, AmazonS3Builder},
    ObjectStore,
};

pub struct S3Storage {
    client: AmazonS3,
}

impl S3Storage {
    pub fn new(
        region: &String,
        url: &String,
        bucket_name: &String,
        access_key_id: &String,
        secret_access_key: &String,
        allow_http: bool,
    ) -> Result<Self, anyhow::Error> {
        let client = AmazonS3Builder::new()
            .with_endpoint(url)
            .with_bucket_name(bucket_name)
            .with_region(region)
            .with_allow_http(allow_http)
            .with_access_key_id(access_key_id)
            .with_secret_access_key(secret_access_key)
            .with_conditional_put(object_store::aws::S3ConditionalPut::ETagMatch)
            .build()?;

        Ok(Self { client })
    }

    pub async fn get(&self, key: PathBuf) -> Result<Bytes, object_store::Error> {
        let path = object_store::path::Path::from_filesystem_path(&key)?;
        // let path = object_store::path::Path::from(key);
        let get_result = self.client.get(&path).await?;
        let res = get_result.bytes().await?;

        Ok(res)
    }

    pub async fn put(
        &self,
        key: PathBuf,
        object: Option<Bytes>,
    ) -> Result<(), object_store::Error> {
        let path = object_store::path::Path::from_filesystem_path(key)?;

        if let Some(object) = object {
            self.client.put(&path, object.into()).await.map(|_| ())?;
            return Ok(());
        }

        self.client.delete(&path).await?;
        Ok(())
    }
}
