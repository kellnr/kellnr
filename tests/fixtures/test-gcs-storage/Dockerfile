# fake-gcs-server, built from upstream source with kellnr's GCS XML API patch.
#
# Upstream does not implement the GCS XML API object routes
# (https://github.com/fsouza/fake-gcs-server/issues/1164), but `object_store`, the GCS
# client kellnr uses, speaks the XML API exclusively. `object_store`'s own CI works around
# this with a prebuilt fork image. Kellnr builds the binary here instead so the tests
# depend only on the public upstream repository, and so the image builds natively on every
# architecture (the fork image is amd64 only).
#
# The GCS tests build this image through crates/storage/tests/gcs_image/mod.rs, which
# crates/kellnr/tests/storage_backends.rs includes as well.

ARG GO_VERSION=1.26
ARG FAKE_GCS_SERVER_VERSION=v1.56.1

FROM golang:${GO_VERSION}-alpine AS build

ARG FAKE_GCS_SERVER_VERSION

RUN apk add --no-cache git
WORKDIR /src
RUN git clone --depth 1 --branch ${FAKE_GCS_SERVER_VERSION} \
    https://github.com/fsouza/fake-gcs-server.git .

# The XML API object handlers, plus the three route registrations that hook them up.
COPY xml_object.go fakestorage/xml_object.go
COPY xml-routes.patch /tmp/xml-routes.patch
RUN git apply --verbose /tmp/xml-routes.patch

RUN CGO_ENABLED=0 go build -o /fake-gcs-server .

FROM alpine:3.22

# Set build-time arguments for the bucket names
ARG CRATES_BUCKET=kellnr-crates
ARG CRATESIO_BUCKET=kellnr-cratesio
ARG TOOLCHAIN_BUCKET=kellnr-toolchains
ARG DOCS_BUCKET=kellnr-docs

COPY --from=build /fake-gcs-server /bin/fake-gcs-server

# fake-gcs-server pre-seeds buckets from directories under /data
RUN mkdir -p /data/${CRATES_BUCKET} /data/${CRATESIO_BUCKET} /data/${TOOLCHAIN_BUCKET} /data/${DOCS_BUCKET}

# Run in HTTP mode (no TLS) so kellnr can talk to it without a trusted cert, and skip
# request signing on the kellnr side (fake-gcs-server does not validate auth by default).
#
# The flat, GCS-XML-API-style object routes dispatch only for requests whose host matches
# `-public-host`. Without a port in it, only the host name is compared, so clients connect
# through `localhost` on any published port (see crates/storage/tests/gcs_image/mod.rs).
ENTRYPOINT ["/bin/fake-gcs-server", "-data", "/data", "-scheme", "http"]
CMD ["-public-host", "localhost"]
