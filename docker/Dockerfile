#################
#  Build image  #
#################

FROM rust:slim-bookworm AS builder

ARG VERSION # Set with --build-arg VERSION="x.y.z"
ARG DEBIAN_FRONTEND=noninteractive
ARG UI_FILE="/sources/crates/web_ui/src/ui.rs"
ENV TZ=Europe/Berlin

RUN apt update
RUN apt install -y \
    git \
    curl \
    sed \
    build-essential \
    pkg-config \
    openssl \
    libssl-dev \
    nodejs \
    npm \
    gcc 

COPY . /sources

# Build the UI
WORKDIR /sources/ui
RUN npm install -g npm@latest
RUN npm install
RUN npm run build

# Build kellnr
WORKDIR /sources
RUN pwd
RUN ls -lah
RUN sed -i -e 's/0.0.0-debug/'"$VERSION"'/g' "$UI_FILE"
RUN cargo build --release


##################
# Runtime image  #
##################

FROM rust:slim-bookworm

RUN apt update
RUN apt install -y \ 
	curl \
	sed \
	git \
	zip \
	apt-utils \
	build-essential \
	llvm-dev \
	libclang-dev \
	clang \
	cmake

RUN mkdir -p /opt/kellnr
RUN mkdir -p /opt/kdata
WORKDIR /opt/kellnr
COPY --from=builder /sources/target/release/kellnr ./kellnr
COPY --from=builder /sources/config/default.toml ./config/default.toml
COPY --from=builder /sources/ui/dist ./static

HEALTHCHECK --interval=5m --timeout=3s CMD curl -f http://localhost:8000/api/v1/ui/version || exit 1

EXPOSE 8000

ENTRYPOINT ["/bin/sh", "-c" , "update-ca-certificates && ./kellnr"]
