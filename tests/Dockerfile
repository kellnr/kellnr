##################################################
#  Docker image for integration tests of Kellnr  #
#                                                #
#  Build with:                                   #
#  docker build -t kellnr-test:local ../         #
##################################################

#################
#  Build image  #
#################

FROM rust:slim-bookworm AS builder

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

# Install latest nodejs version with nvm
ENV NVM_DIR="/root/.nvm"
ENV NODE_VERSION="24.5.0"
RUN curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.1/install.sh | bash \
  && . "$NVM_DIR/nvm.sh" \
  && nvm install $NODE_VERSION \
  && nvm alias default $NODE_VERSION \
  && nvm use $NODE_VERSION
ENV NODE_PATH="$NVM_DIR/versions/node/$NODE_VERSION/lib/node_modules"
ENV PATH="$NVM_DIR/versions/node/v$NODE_VERSION/bin:$HOME/.cargo/bin:$PATH"
COPY . /sources

# Build kellnr
WORKDIR /sources
RUN just build-release 


##################
# Runtime image  #
##################

FROM rust:slim-bookworm

RUN mkdir -p /opt/kellnr
RUN rustup component add rust-src
WORKDIR /opt/kellnr
COPY --from=builder /sources/target/release/kellnr ./kellnr
# The next line can be removed in future releases if the default config is baked into the binary
# It is currently needed, for the migration test from a version that needs the config file (5.8.0 and earlier)
COPY --from=builder /sources/config/default.toml ./config/default.toml

COPY tests/ca.crt /usr/local/share/ca-certificates/ca.crt
RUN update-ca-certificates

HEALTHCHECK --interval=5m --timeout=3s CMD curl -f http://localhost:8000/api/v1/health || exit 1

EXPOSE 8000

ENTRYPOINT ["/bin/sh", "-c" , "update-ca-certificates && ./kellnr"]
