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

# Install latest nodejs version with nvm
ENV NVM_DIR="/root/.nvm"
ENV NODE_VERSION="23.2.0"
RUN curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.1/install.sh | bash \
    && . "$NVM_DIR/nvm.sh" \
    && nvm install $NODE_VERSION \
    && nvm alias default $NODE_VERSION \
    && nvm use $NODE_VERSION
ENV NODE_PATH="$NVM_DIR/versions/node/$NODE_VERSION/lib/node_modules"
ENV PATH="$NVM_DIR/versions/node/v$NODE_VERSION/bin:$PATH"
COPY . /sources

# Build the UI
WORKDIR /sources/ui
RUN npm install
RUN npm run build


# Build kellnr
WORKDIR /sources
RUN cargo build --release --features "vendored-openssl"


##################
# Runtime image  #
##################

FROM rust:slim-bookworm

RUN mkdir -p /opt/kellnr
RUN rustup component add rust-src
WORKDIR /opt/kellnr
COPY --from=builder /sources/target/release/kellnr ./kellnr
COPY --from=builder /sources/config/default.toml ./config/default.toml
COPY --from=builder /sources/ui/dist ./static

COPY tests/ca.crt /usr/local/share/ca-certificates/ca.crt
RUN update-ca-certificates

HEALTHCHECK --interval=5m --timeout=3s CMD curl -f http://localhost:8000/api/v1/ui/version || exit 1

EXPOSE 8000

ENTRYPOINT ["/bin/sh", "-c" , "update-ca-certificates && ./kellnr"]
