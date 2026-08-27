use std::borrow::Cow;

use testcontainers::Image;
use testcontainers::core::wait::HttpWaitStrategy;
use testcontainers::core::{ContainerPort, WaitFor};

// The upstream `fsouza/fake-gcs-server` image doesn't implement the plain XML-style
// `PUT /{bucket}/{object}` request that `object_store`'s GCS client sends (it 404s/405s
// on it, see https://github.com/fsouza/fake-gcs-server/issues/1164). `object_store`'s own
// CI works around this with a patched fork image, referenced directly in their workflow
// (https://github.com/apache/arrow-rs-object-store/blob/main/.github/workflows/ci.yml).
// Use the same fork here.
const NAME: &str = "tustvold/fake-gcs-server";
const TAG: &str = "latest";

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

    pub fn new(host_port: u16) -> Self {
        Self { host_port }
    }
}

impl Image for FakeGcsServer {
    fn name(&self) -> &str {
        NAME
    }

    fn tag(&self) -> &str {
        TAG
    }

    fn ready_conditions(&self) -> Vec<WaitFor> {
        // The bucket-listing endpoint responds with 200 once the server is up.
        vec![WaitFor::http(
            HttpWaitStrategy::new("/storage/v1/b")
                .with_port(Self::CONTAINER_PORT)
                .with_expected_status_code(200_u16),
        )]
    }

    fn entrypoint(&self) -> Option<&str> {
        // Override entrypoint to create the bucket directories before starting.
        Some("sh")
    }

    fn cmd(&self) -> impl IntoIterator<Item = impl Into<Cow<'_, str>>> {
        vec![
            "-c".to_string(),
            format!(
                "mkdir -p /data/kellnr-crates /data/kellnr-cratesio /data/kellnr-toolchains && \
                 exec /bin/fake-gcs-server -data /data -scheme http -public-host localhost:{}",
                self.host_port
            ),
        ]
    }

    fn expose_ports(&self) -> &[ContainerPort] {
        &[Self::CONTAINER_PORT]
    }
}
