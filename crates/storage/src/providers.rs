use bytes::Bytes;
use object_store::{
    aws::{AmazonS3, AmazonS3Builder},
    ObjectStore,
};

pub struct S3Storage {
    pub enabled: bool,
    client: AmazonS3,
}

impl S3Storage {
    pub fn new(
        enabled: bool,
        region: String,
        url: String,
        bucket_name: String,
        access_key_id: String,
        secret_access_key: String,
        allow_http: bool,
    ) -> Result<Self, anyhow::Error> {
        let client = AmazonS3Builder::new()
            .with_endpoint(url)
            .with_bucket_name(bucket_name)
            .with_region(region)
            .with_allow_http(allow_http)
            .with_access_key_id(access_key_id)
            .with_secret_access_key(secret_access_key)
            .build()?;

        Ok(Self { enabled, client })
    }
}

pub struct CrateObject(pub bytes::Bytes);

impl From<Bytes> for CrateObject {
    fn from(value: Bytes) -> Self {
        Self(value)
    }
}

impl From<CrateObject> for Bytes {
    fn from(value: CrateObject) -> Self {
        value.0
    }
}

pub struct CrateId(String);

impl CrateId {
    pub fn path(self) -> String {
        self.0
    }
    pub fn new(location: String) -> Self {
        Self(location)
    }
}

pub trait StorageObject: From<Bytes>
where
    Bytes: From<Self>,
{
}

impl StorageObject for CrateObject {}

pub trait StorageProvider: Send + Sync + 'static
where
    bytes::Bytes: From<<Self as StorageProvider>::Object>,
{
    type Object: StorageObject;
    type ObjectId;

    async fn get(&self, by: Self::ObjectId) -> Result<Self::Object, object_store::Error>;
    async fn put(
        &self,
        object: Option<Self::Object>,
        by: Self::ObjectId,
    ) -> Result<(), object_store::Error>;
}

impl StorageProvider for S3Storage {
    type Object = CrateObject;

    type ObjectId = CrateId;

    async fn get(&self, by: Self::ObjectId) -> Result<Self::Object, object_store::Error> {
        let path = object_store::path::Path::from(by.path());
        let get_result = self.client.get(&path).await?;
        let res = get_result.bytes().await?.into();

        Ok(res)
    }

    async fn put(
        &self,
        object: Option<Self::Object>,
        by: Self::ObjectId,
    ) -> Result<(), object_store::Error> {
        let path = object_store::path::Path::from(by.path());

        if let Some(object) = object {
            self.client.put(&path, object.0.into()).await.map(|_| ())?;
            return Ok(());
        }

        self.client.delete(&path).await?;
        Ok(())
    }
}

pub struct S3NoopStorage;

impl StorageProvider for S3NoopStorage {
    type Object = CrateObject;

    type ObjectId = CrateId;

    async fn get(&self, by: Self::ObjectId) -> Result<Self::Object, object_store::Error> {
        Ok(bytes::Bytes::new().into())
    }

    async fn put(
        &self,
        object: Option<Self::Object>,
        by: Self::ObjectId,
    ) -> Result<(), object_store::Error> {
        Ok(())
    }
}


pub mod fs {

    use object_store::local::LocalFileSystem;

    use super::{StorageObject, StorageProvider};
    use object_store::ObjectStore;

    pub struct FsObject(pub bytes::Bytes);

    pub struct FsId(pub String);

    impl From<bytes::Bytes> for FsObject {
        fn from(value: bytes::Bytes) -> Self {
            Self(value)
        }
    }

    impl From<FsObject> for bytes::Bytes {
        fn from(value: FsObject) -> Self {
            value.0
        }
    }

    impl StorageObject for FsObject {}

    pub struct FsStorage(pub LocalFileSystem);
    impl FsStorage {
        pub fn new() -> Self {
            let local_fs = LocalFileSystem::new();
            Self(local_fs)
        }
    }

    impl Default for FsStorage {
        fn default() -> Self {
            Self::new()
        }
    }

    impl StorageProvider for FsStorage {
        type Object = FsObject;

        type ObjectId = FsId;

        async fn get(&self, by: Self::ObjectId) -> Result<Self::Object, object_store::Error> {
            let path = object_store::path::Path::from(by.0);
            let get_result = self.0.get(&path).await?;
            let res = get_result.bytes().await?.into();

            Ok(res)
        }

        async fn put(
            &self,
            object: Option<Self::Object>,
            by: Self::ObjectId,
        ) -> Result<(), object_store::Error> {
            let path = object_store::path::Path::from(by.0);

            if let Some(object) = object {
                self.0.put(&path, object.0.into()).await.map(|_| ())?;
                return Ok(());
            }

            self.0.delete(&path).await?;
            Ok(())
        }
    }
}
