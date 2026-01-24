##################################################
#  Docker image for integration tests of Kellnr  #
#                                                #
#  Build with (from repo root):                  #
#  docker build -f tests/Dockerfile -t kellnr-test:local --build-arg KELLNR_VERSION=local . #
##################################################

#################
#  Build image  #
#################

FROM rust:slim-trixie AS builder

ARG DEBIAN_FRONTEND=noninteractive
ENV TZ=Europe/Berlin

ARG KELLNR_VERSION="0.0.0-unknown"
ENV KELLNR_VERSION=$KELLNR_VERSION

RUN apt update
RUN apt install -y -q --no-install-recommends \
    ca-certificates \
    git \
    curl \
    sed \
    build-essential \
    pkg-config \
    openssl \
    libssl-dev \
    gcc \
    && rm -rf /var/lib/apt/lists/*

# Install just command runner
RUN cargo install just

# Node is required by `just build-release` in this repository (build scripts / tooling).
# Keep this identical to the previous test image setup.
ENV NVM_DIR="/root/.nvm"
ENV NODE_VERSION="24.5.0"
RUN curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.1/install.sh | bash \
    && . "$NVM_DIR/nvm.sh" \
    && nvm install $NODE_VERSION \
    && nvm alias default $NODE_VERSION \
    && nvm use $NODE_VERSION
ENV NODE_PATH="$NVM_DIR/versions/node/$NODE_VERSION/lib/node_modules"
ENV PATH="$NVM_DIR/versions/node/v$NODE_VERSION/bin:/root/.cargo/bin:$PATH"

# IMPORTANT:
# Build context is the *repo root*. This Dockerfile lives in tests/, but we need
# the Kellnr source tree (justfile, crates, config, etc.) to build the binary.
COPY . /sources

# Build kellnr
WORKDIR /sources
RUN just npm-build
RUN just build


##################
# Runtime image  #
##################

FROM rust:slim-trixie

RUN mkdir -p /opt/kellnr
RUN rustup component add rust-src
WORKDIR /opt/kellnr
COPY --from=builder /sources/target/debug/kellnr ./kellnr

# Self-contained cert: copy from tests/ (from repo root build context).
COPY tests/ca.crt /usr/local/share/ca-certificates/ca.crt
RUN update-ca-certificates

HEALTHCHECK --interval=5m --timeout=3s CMD curl -f http://localhost:8000/api/v1/health || exit 1

EXPOSE 8000

ENTRYPOINT ["/bin/sh", "-c" , "./kellnr"]
