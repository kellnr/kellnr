#!/bin/bash
echo && \
echo 'Final install versions for the record' && \
for pkg in git git-lfs vim vim-tiny net-tools iputils-ping dnsutils bash-completion nano tree curl \
            unzip apt-transport-https ca-certificates gnupg libtool autoconf automake pkg-config gcc \
            cmake ninja-build autogen libtool libgtest-dev libboost-all-dev build-essential libapr1-dev \
            libaprutil1-dev libssl-dev openssl xz-utils just \
            nodejs \
            docker-ce-cli docker-buildx-plugin docker-compose-plugin \
            ; do \
echo -n $pkg && apt-cache policy $pkg | grep Installed; done && \
cargo -V && \
echo "node $(node -v)" && \
echo "npm $(npm -v)" && \
echo && \
echo 'Final ENVs for the record' && \
export && \
echo

exit 0
