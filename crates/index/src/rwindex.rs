use super::config_json::ConfigJson;
use anyhow::Result;
use common::index_metadata::IndexMetadata;
use common::prefetch::Prefetch;
use rocket::async_trait;

#[async_trait]
pub trait RoIndex: Send + Sync {
    fn get_config(&self) -> ConfigJson;
    async fn get_prefetch_data(&self, package: &str) -> Result<Prefetch>;
}

#[async_trait]
pub trait WoIndex: Send + Sync {
    async fn add_to_index(&self, metadata: &IndexMetadata) -> Result<()>;
    async fn yank(&self, crate_name: &str, version: &str, yanked: bool) -> Result<()>;
    async fn delete(&self, crate_name: &str, version: &str) -> Result<()>;
}

pub trait RwIndex: WoIndex + RoIndex {}

pub mod mock {
    use super::*;
    use mockall::*;

    // Gets converted to "MockIdx" from automock
    mock! {
        pub Idx {}

        impl RwIndex for Idx {}

        #[async_trait]
        impl RoIndex for Idx {
            fn get_config(&self) -> ConfigJson {
                unimplemented!()
            }

            async fn get_prefetch_data(&self, _package: &str) -> Result<Prefetch> {
                unimplemented!();
            }
        }

        #[async_trait]
        impl WoIndex for Idx {
            async fn add_to_index(&self, _metadata: &IndexMetadata) -> Result<()> {
                unimplemented!()
            }

            async fn yank(&self, _crate_name: &str, _version: &str, _yanked: bool) -> Result<()> {
                unimplemented!()
            }

            async fn delete(&self, _crate_name: &str, _version: &str) -> Result<()> {
                unimplemented!()
            }
        }
    }
}
