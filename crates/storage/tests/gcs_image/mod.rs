use std::borrow::Cow;
use std::path::PathBuf;

use testcontainers::core::wait::HttpWaitStrategy;
use testcontainers::core::{
    BuildContextBuilder, ContainerPort, CopyToContainerCollection, WaitFor,
};
use testcontainers::{BuildableImage, Image};

// Built from source rather than pulled: upstream `fsouza/fake-gcs-server` doesn't
// implement the plain XML-style `PUT /{bucket}/{object}` request that `object_store`'s GCS
// client sends (see https://github.com/fsouza/fake-gcs-server/issues/1164), and the fork
// image `object_store`'s own CI uses is a personal, amd64-only `latest` tag. The Dockerfile
// under `tests/fixtures/test-gcs-storage` builds a pinned upstream release with kellnr's
// XML API patch applied, which also gives a native image on arm64. The Playwright UI tests
// build the very same Dockerfile, see `tests/src/lib/docker.ts`.
const IMAGE_NAME: &str = "kellnr-fake-gcs-server";
const IMAGE_TAG: &str = "local";

/// Build context for [`FakeGcsServer`], shared with the Playwright UI tests.
pub(crate) struct FakeGcsServerImage;

impl FakeGcsServerImage {
    fn fixture_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/test-gcs-storage")
    }
}

impl BuildableImage for FakeGcsServerImage {
    type Built = FakeGcsServer;

    fn build_context(&self) -> CopyToContainerCollection {
        let dir = Self::fixture_dir();
        BuildContextBuilder::default()
            .with_dockerfile(dir.join("Dockerfile"))
            .with_file(dir.join("xml_object.go"), "./xml_object.go")
            .with_file(dir.join("xml-routes.patch"), "./xml-routes.patch")
            .as_copy_to_container_collection()
    }

    fn descriptor(&self) -> String {
        format!("{IMAGE_NAME}:{IMAGE_TAG}")
    }

    fn into_image(self) -> Self::Built {
        FakeGcsServer
    }
}

/// `fake-gcs-server` dispatches its flat, GCS-XML-API-style object routes (used by
/// `object_store`) only for requests whose host matches `-public-host`. Without a port in
/// `-public-host`, only the host name is compared, so the container can be published on
/// any free host port that Docker picks. Clients must connect through `localhost`.
#[derive(Debug, Clone)]
pub(crate) struct FakeGcsServer;

impl FakeGcsServer {
    pub(crate) const PORT: u16 = 4443;
    pub(crate) const CONTAINER_PORT: ContainerPort = ContainerPort::Tcp(Self::PORT);
}

impl Image for FakeGcsServer {
    fn name(&self) -> &str {
        IMAGE_NAME
    }

    fn tag(&self) -> &str {
        IMAGE_TAG
    }

    fn ready_conditions(&self) -> Vec<WaitFor> {
        // The bucket-listing endpoint responds with 200 once the server is up.
        vec![WaitFor::http(
            HttpWaitStrategy::new("/storage/v1/b")
                .with_port(Self::CONTAINER_PORT)
                .with_expected_status_code(200_u16),
        )]
    }

    fn cmd(&self) -> impl IntoIterator<Item = impl Into<Cow<'_, str>>> {
        // The image's ENTRYPOINT already carries `-data /data -scheme http`, and the bucket
        // directories are created at build time.
        vec!["-public-host", "localhost"]
    }

    fn expose_ports(&self) -> &[ContainerPort] {
        &[Self::CONTAINER_PORT]
    }
}
