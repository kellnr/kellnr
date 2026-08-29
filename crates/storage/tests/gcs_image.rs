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
pub struct FakeGcsServerImage {
    host_port: u16,
}

impl FakeGcsServerImage {
    pub fn new(host_port: u16) -> Self {
        Self { host_port }
    }

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
        FakeGcsServer {
            host_port: self.host_port,
        }
    }
}

/// `fake-gcs-server`'s `-public-host` flag must match the exact `host:port` the client
/// connects through for its flat, GCS-XML-API-style object routes (used by
/// `object_store`) to dispatch correctly; a mismatch (e.g. a Docker-assigned random host
/// port that differs from a hardcoded `-public-host`) causes every GET/PUT to 404/405.
/// The host port is therefore reserved up front and threaded through here so it can be
/// baked into `-public-host`, and the container is started with that same port fixed via
/// `ImageExt::with_mapped_port` (see `fakegcs-testcontainer`).
#[derive(Debug, Clone)]
pub struct FakeGcsServer {
    host_port: u16,
}

impl FakeGcsServer {
    pub const PORT: u16 = 4443;
    pub const CONTAINER_PORT: ContainerPort = ContainerPort::Tcp(Self::PORT);
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
        vec![
            "-public-host".to_string(),
            format!("localhost:{}", self.host_port),
        ]
    }

    fn expose_ports(&self) -> &[ContainerPort] {
        &[Self::CONTAINER_PORT]
    }
}
