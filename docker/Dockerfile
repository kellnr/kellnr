FROM rust:1.79.0-bookworm as rust_builder


RUN apt update && \
    apt install -y \
      apt-utils \
      build-essential \
      clang \
      cmake \
      curl \
      git \
      libclang-dev \
      llvm-dev \
      nodejs \
      npm \
      sed \
      zip

COPY . /sources
WORKDIR /sources/crates/kellnr

RUN cargo build --release

FROM rust:1.79.0-bookworm

RUN apt update && \
    apt install -y \
      apt-utils \
      build-essential \
      clang \
      cmake \
      curl \
      git \
      libclang-dev \
      llvm-dev \
      sed \
      zip

COPY --from=rust_builder /sources/target/release/kellnr /usr/bin
COPY --from=rust_builder /sources/config/default.toml /config/default.toml
COPY --from=rust_builder /sources/static /static

EXPOSE 8000

HEALTHCHECK --interval=5m --timeout=3s CMD curl -f http://localhost:8000/api/v1/ui/version || exit 1
CMD kellnr
