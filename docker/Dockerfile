##################
# Runtime image  #
##################

FROM rust:slim-trixie

ARG VERSION

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
    cmake \
    pkg-config \
    libssl-dev

RUN curl -sL https://github.com/kellnr/installer/releases/download/v0.1.2/install.sh -o /tmp/install.sh && \
    bash /tmp/install.sh -v ${VERSION} && \
    rm /tmp/install.sh
RUN rustup component add rust-src

ENV KELLNR_CONFIG_FILE=/etc/kellnr/kellnr.toml
ENV KELLNR_SETUP__ADMIN_PWD=admin

HEALTHCHECK --interval=5m --timeout=3s CMD curl -f http://localhost:8000/api/v1/health || exit 1

EXPOSE 8000

ENTRYPOINT ["/bin/sh", "-c" , "update-ca-certificates && kellnr start"]
