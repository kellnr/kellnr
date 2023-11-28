FROM ubuntu:22.04

ARG ADMINPWD=admin
ENV INSTALLDIR=/opt
ENV DATADIR=/opt/kdata
ARG AUTHTOKEN=Zy9HhJ02RJmg0GCrgLfaCVfU6IwDfhXD
ARG VERSION

RUN apt update
RUN apt install -y curl sed git zip apt-utils build-essential

# Install rust for the automatic rustdoc generation
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
ENV PATH="/root/.cargo/bin:${PATH}"

COPY start.sh ${INSTALLDIR}
RUN chmod u+x ${INSTALLDIR}/start.sh

WORKDIR ${INSTALLDIR}
RUN curl -s https://raw.githubusercontent.com/kellnr/installer/main/install.sh | bash -s -- -d ${DATADIR} -p ${ADMINPWD} -t ${AUTHTOKEN} -v ${VERSION}

ENTRYPOINT ["/opt/start.sh"]
