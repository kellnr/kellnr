##################################################
#  Docker image for integration tests of Kellnr   #
#                                                #
#  Goal: maximize cache reuse on CI               #
#  - Build in debug mode (faster)                 #
#  - Use BuildKit cache mounts                    #
#  - Separate dependency-heavy layers             #
#                                                #
#  Build with (from repo root):                   #
#  DOCKER_BUILDKIT=1 docker build                 #
#    -f tests/Dockerfile                          #
#    -t kellnr-test:local                         #
#    --build-arg KELLNR_VERSION=local             #
#    .                                            #
##################################################

#################
#  Build image  #
#################

FROM rust:slim-bookworm AS builder

ARG DEBIAN_FRONTEND=noninteractive
ENV TZ=Europe/Berlin

ARG KELLNR_VERSION="0.0.0-unknown"
ENV KELLNR_VERSION=$KELLNR_VERSION

# OS dependencies required to compile Rust + build the embedded UI
RUN apt update && apt install -y -q --no-install-recommends \
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

# Install just (cache cargo downloads)
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo install just

# Node (via nvm) is required because the Rust binary embeds built UI assets.
ENV NVM_DIR="/root/.nvm"
ENV NODE_VERSION="24.5.0"
RUN curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.1/install.sh | bash \
  && . "$NVM_DIR/nvm.sh" \
  && nvm install $NODE_VERSION \
  && nvm alias default $NODE_VERSION \
  && nvm use $NODE_VERSION
ENV NODE_PATH="$NVM_DIR/versions/node/$NODE_VERSION/lib/node_modules"
ENV PATH="$NVM_DIR/versions/node/v$NODE_VERSION/bin:/root/.cargo/bin:$PATH"

WORKDIR /sources

############################################
# 1) Cache NPM dependencies (ui/)          #
############################################

# Copy only the UI package inputs first so `npm install` can be cached across backend changes.
COPY ui/package.json ui/package-lock.json ./ui/

# Install UI dependencies into ui/node_modules (cache mount survives across builds with GHA cache).
RUN --mount=type=cache,target=/sources/ui/node_modules \
    cd ui && npm ci

############################################################
# 2) Cache Rust dependencies (Cargo + workspace crates)     #
############################################################

# Copy top-level cargo files first
COPY Cargo.toml Cargo.lock ./

# Copy the workspace crates directory (including sources).
#
# This is still more cache-friendly than copying the entire repository at once, because it avoids
# invalidating Rust compilation due to changes outside `crates/` (e.g. `tests/`, `.github/`).
# Note: changes within `crates/` will still invalidate the following build layer.
COPY crates ./crates

# Warm Rust caches (debug) to populate /sources/target and cargo caches.
# IMPORTANT: the `kellnr` crate embeds `ui/dist` at compile time, which does not exist yet here.
# So we warm caches by compiling the workspace *excluding* `kellnr`.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/sources/target \
    cargo build --workspace --exclude kellnr --features vendored-openssl

############################################
# 3) Build UI + final debug binary         #
############################################

# Now copy the full repository (this invalidates frequently, but earlier layers remain cached).
# Keep this late so most expensive work stays cacheable.
COPY . /sources

# Build UI (uses cached node_modules) and build the final debug binary (uses cached target/cargo).
# `build-debug` runs `npm-build` then `cargo build` in debug mode.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/sources/target \
    --mount=type=cache,target=/sources/ui/node_modules \
    just build-debug

##################
# Runtime image  #
##################

FROM rust:slim-bookworm

# Runtime deps for healthcheck + certs
RUN apt update && apt install -y -q --no-install-recommends \
  ca-certificates \
  curl \
  && rm -rf /var/lib/apt/lists/*

RUN mkdir -p /opt/kellnr
RUN rustup component add rust-src

WORKDIR /opt/kellnr
COPY --from=builder /sources/target/debug/kellnr ./kellnr

# Self-contained cert: copy from tests/ (from repo root build context).
COPY tests/ca.crt /usr/local/share/ca-certificates/ca.crt
RUN update-ca-certificates

HEALTHCHECK --interval=5m --timeout=3s CMD curl -f http://localhost:8000/api/v1/health || exit 1

EXPOSE 8000

ENTRYPOINT ["/bin/sh", "-c" , "update-ca-certificates && ./kellnr"]
