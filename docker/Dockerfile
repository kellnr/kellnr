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

# Capture the toolchain installed by the base image so the entrypoint can
# pin RUSTUP_TOOLCHAIN to it. Prevents a rust-toolchain.toml shipped inside
# a published crate (or a transitive dependency) from hijacking rustc
# during docs generation. See issue #1176.
RUN mkdir -p /etc/kellnr && \
    rustup show active-toolchain | awk '{print $1}' > /etc/kellnr/active-toolchain

# Kellnr runs as an unprivileged user. The entrypoint starts as root to
# update the CA certificates and to take over root-owned data volumes of
# older images, then drops privileges.
# The base image makes the toolchain world-writable. Docs builds run crate
# code as the kellnr user, so the toolchain must stay read-only for it, and
# cargo gets its own writable CARGO_HOME.
RUN groupadd --gid 1000 kellnr && \
    useradd --uid 1000 --gid kellnr --create-home --home-dir /home/kellnr \
        --shell /usr/sbin/nologin kellnr && \
    chmod -R go-w "$RUSTUP_HOME" "$CARGO_HOME" && \
    mkdir -p /var/lib/kellnr && \
    chown -R kellnr:kellnr /var/lib/kellnr
ENV CARGO_HOME=/home/kellnr/.cargo

COPY docker/entrypoint.sh /usr/local/bin/kellnr-entrypoint.sh
RUN chmod +x /usr/local/bin/kellnr-entrypoint.sh

ENV KELLNR_CONFIG_FILE=/etc/kellnr/kellnr.toml
ENV KELLNR_SETUP__ADMIN_PWD=admin

HEALTHCHECK --interval=5m --timeout=3s CMD curl -f http://localhost:8000/api/v1/health || exit 1

EXPOSE 8000

ENTRYPOINT ["/usr/local/bin/kellnr-entrypoint.sh"]
