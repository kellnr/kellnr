##################
# Runtime image  #
##################

FROM rust:slim-trixie

ARG ADMINPWD=admin
ARG AUTHTOKEN=Zy9HhJ02RJmg0GCrgLfaCVfU6IwDfhXD
ARG VERSION
ENV KELLNR_CONFIG_FILE=/etc/kellnr/kellnr.toml

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

WORKDIR ${INSTALLDIR}
RUN curl -s https://raw.githubusercontent.com/kellnr/installer/main/install.sh | bash -s -- -p ${ADMINPWD} -t ${AUTHTOKEN} -v ${VERSION}
RUN rustup component add rust-src

HEALTHCHECK --interval=5m --timeout=3s CMD curl -f http://localhost:8000/api/v1/health || exit 1

EXPOSE 8000

ENTRYPOINT ["/bin/sh", "-c" , "update-ca-certificates && kellnr run"]
