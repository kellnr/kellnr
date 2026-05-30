#!/bin/sh
set -e

update-ca-certificates

# Pin rustup to the toolchain baked into this image so a published
# rust-toolchain.toml in an uploaded crate or a transitive dependency
# can't silently switch the compiler during docs generation. The exact
# version is captured at image build time. See issue #1176.
if [ -r /etc/kellnr/active-toolchain ]; then
    RUSTUP_TOOLCHAIN=$(cat /etc/kellnr/active-toolchain)
    export RUSTUP_TOOLCHAIN
fi

exec kellnr start "$@"
