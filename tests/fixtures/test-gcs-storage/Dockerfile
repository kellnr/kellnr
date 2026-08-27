# The upstream `fsouza/fake-gcs-server` image doesn't implement the plain XML-style
# `PUT /{bucket}/{object}` request that `object_store`'s GCS client sends (it 404s/405s
# on it, see https://github.com/fsouza/fake-gcs-server/issues/1164). `object_store`'s own
# CI works around this with a patched fork image, referenced directly in their workflow
# (https://github.com/apache/arrow-rs-object-store/blob/main/.github/workflows/ci.yml).
# Use the same fork here (see also crates/storage/tests/gcs_image.rs, which hit and fixed
# the identical issue on the Rust integration-test side).
FROM tustvold/fake-gcs-server:latest

# Set build-time arguments for the bucket names
ARG CRATES_BUCKET=kellnr-crates
ARG CRATESIO_BUCKET=kellnr-cratesio
ARG TOOLCHAIN_BUCKET=kellnr-toolchains

# fake-gcs-server pre-seeds buckets from directories under /data
RUN mkdir -p /data/${CRATES_BUCKET} /data/${CRATESIO_BUCKET} /data/${TOOLCHAIN_BUCKET}

# Run in HTTP mode (no TLS) so kellnr can talk to it without a trusted cert, and skip
# request signing on the kellnr side (fake-gcs-server does not validate auth by default).
#
# `-public-host` must match the exact host:port the client connects through for the flat,
# GCS-XML-API-style object routes to dispatch correctly (a mismatch causes 404/405 on every
# GET/PUT). Since that host:port (a Docker-assigned host port) isn't known at build time,
# it's kept out of ENTRYPOINT and left as an overridable CMD default so callers that need a
# real, dynamically-reserved host port can replace it (see startGcsFakeServerContainer in
# tests/src/lib/docker.ts, which mirrors the same fix applied to the Rust test image).
ENTRYPOINT ["/bin/fake-gcs-server", "-data", "/data", "-scheme", "http"]
CMD ["-public-host", "fake-gcs-server:4443"]
