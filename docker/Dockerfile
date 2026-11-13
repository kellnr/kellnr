# The kellnr binary comes from the GitHub release of VERSION. The release
# workflow verifies the attestation of each zip and passes its digest, so the
# download is pinned and recorded in the image provenance.
ARG VERSION
ARG KELLNR_SHA256_AMD64
ARG KELLNR_SHA256_ARM64

FROM scratch AS kellnr-zip-amd64
ARG VERSION
ARG KELLNR_SHA256_AMD64
ADD --checksum=sha256:${KELLNR_SHA256_AMD64} \
    https://github.com/kellnr/kellnr/releases/download/v${VERSION}/kellnr-x86_64-unknown-linux-gnu.zip /kellnr.zip

FROM scratch AS kellnr-zip-arm64
ARG VERSION
ARG KELLNR_SHA256_ARM64
ADD --checksum=sha256:${KELLNR_SHA256_ARM64} \
    https://github.com/kellnr/kellnr/releases/download/v${VERSION}/kellnr-aarch64-unknown-linux-gnu.zip /kellnr.zip

FROM kellnr-zip-${TARGETARCH} AS kellnr-zip

FROM --platform=$BUILDPLATFORM alpine:3.23 AS kellnr-bin
COPY --from=kellnr-zip /kellnr.zip /tmp/kellnr.zip
RUN unzip -q /tmp/kellnr.zip -d /out && chmod 0755 /out/kellnr

##################
# Runtime image  #
##################

FROM rust:slim-trixie

RUN apt update
RUN apt install -y \
    curl \
    git \
    build-essential \
    llvm-dev \
    libclang-dev \
    clang \
    cmake \
    pkg-config \
    libssl-dev

COPY --from=kellnr-bin /out/kellnr /usr/local/bin/kellnr
COPY docker/kellnr.toml /etc/kellnr/kellnr.toml
RUN rustup component add rust-src

# Capture the toolchain installed by the base image so the entrypoint can
# pin RUSTUP_TOOLCHAIN to it. Prevents a rust-toolchain.toml shipped inside
# a published crate (or a transitive dependency) from hijacking rustc
# during docs generation. See issue #1176.
RUN rustup show active-toolchain | awk '{print $1}' > /etc/kellnr/active-toolchain

# Kellnr runs as an unprivileged user. The entrypoint starts as root to
# update the CA certificates and to take over root-owned data volumes of
# older images, then drops privileges.
# The base image makes the toolchain world-writable. Docs builds run crate
# code as the kellnr user, so the toolchain must stay read-only for it, and
# cargo gets its own writable CARGO_HOME. The data directory is group
# writable, so it works for the kellnr user, for --user 1000:1000 and for
# root without capabilities (the entrypoint then keeps running as root).
RUN groupadd --gid 1000 kellnr && \
    useradd --uid 1000 --gid kellnr --create-home --home-dir /home/kellnr \
        --shell /usr/sbin/nologin kellnr && \
    install -d -o kellnr -g kellnr /home/kellnr/.cargo && \
    chmod -R go-w "$RUSTUP_HOME" "$CARGO_HOME" && \
    install -d -o root -g kellnr -m 0775 /var/lib/kellnr
ENV CARGO_HOME=/home/kellnr/.cargo

COPY docker/entrypoint.sh /usr/local/bin/kellnr-entrypoint.sh
RUN chmod +x /usr/local/bin/kellnr-entrypoint.sh

ENV KELLNR_CONFIG_FILE=/etc/kellnr/kellnr.toml
ENV KELLNR_SETUP__ADMIN_PWD=admin

HEALTHCHECK --interval=5m --timeout=3s CMD curl -f http://localhost:8000/api/v1/health || exit 1

EXPOSE 8000

ENTRYPOINT ["/usr/local/bin/kellnr-entrypoint.sh"]
